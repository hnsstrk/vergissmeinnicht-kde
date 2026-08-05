//! `LlmClient` — blockierender HTTP-Client gegen einen OpenAI-kompatiblen
//! Endpunkt (`/v1/chat/completions`, `/v1/models`), Spec §4.1.
//!
//! JSON-Ausgabe wird zweistufig erzwungen: `response_format` im Request UND
//! eine JSON-Anweisung im Prompt — manche Endpunkte ignorieren
//! `response_format` (Anthropics OpenAI-Kompatibilitätsschicht dokumentiert
//! genau das). Jede Antwort wird unabhängig davon validiert; bei unparsbarer
//! Ausgabe folgt ein stiller Retry mit Format-Erinnerung, danach Fehler.
//!
//! Vor dem Client steht der Trait [`Llm`], damit Tests (und ab AI-A3 die
//! App-Logik) einen Mock einsetzen können.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Fester Request-Timeout — bewusst Konstante, kein Setting: lokale Modelle
/// brauchen beim ersten Aufruf Ladezeit (Spec §4.2).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// JSON-Anweisung, die jedem `complete_json`-Aufruf in die System-Nachricht
/// gelegt wird (zweite Stufe der JSON-Erzwingung).
const JSON_INSTRUCTION: &str = "Antworte ausschließlich mit einem einzigen gültigen JSON-Objekt. \
     Kein Markdown, keine Code-Zäune, kein Text vor oder nach dem JSON.";

/// Format-Erinnerung für den stillen Retry nach unparsbarer Antwort.
const JSON_RETRY_REMINDER: &str = "Deine letzte Antwort war kein gültiges JSON-Objekt. Antworte jetzt \
     ausschließlich mit dem angeforderten JSON-Objekt — ohne Markdown, ohne \
     Erklärungen, ohne weiteren Text.";

/// Fehlerarten des KI-Moduls. Die Unterscheidung braucht Spec §7 für
/// provider-bewusste Meldungen (z. B. „läuft Ollama?" nur bei Netzwerkfehlern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    /// Transportfehler: Verbindung, DNS, Timeout.
    Network(String),
    /// Der Endpunkt hat geantwortet, aber mit Fehlerstatus oder Fehlerobjekt.
    Api(String),
    /// Auch nach dem Format-Retry kein gültiges JSON-Objekt.
    InvalidJson(String),
    /// Lokale Konfiguration oder Secret Service.
    Config(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::Network(e) => write!(f, "Netzwerkfehler: {e}"),
            AiError::Api(e) => write!(f, "KI-Endpunkt: {e}"),
            AiError::InvalidJson(e) => write!(f, "Keine gültige JSON-Antwort: {e}"),
            AiError::Config(e) => write!(f, "KI-Konfiguration: {e}"),
        }
    }
}

/// Eine Chat-Nachricht im OpenAI-Wire-Format (`role`/`content`). Von Anfang
/// an als Array geführt, damit der spätere Chat (Stufe 4) mehrturn-fähig ist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self { role: "system".into(), content: content.into() }
    }

    pub fn user(content: &str) -> Self {
        Self { role: "user".into(), content: content.into() }
    }

    pub fn assistant(content: &str) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

/// Schnittstelle vor dem HTTP-Client, damit Tests einen Mock einsetzen können.
/// `complete_json` ist als Default-Methode implementiert — so läuft die
/// Validierungs- und Retry-Logik auch über dem Mock und ist ohne Netz testbar.
pub trait Llm {
    /// Ein einzelner Chat-Aufruf; liefert den rohen Antworttext des Modells.
    fn chat(&self, messages: &[ChatMessage]) -> Result<String, AiError>;

    /// Modellliste des Endpunkts (`/v1/models`) — füllt die Modellauswahl
    /// der Einstellungsseite und dient „Speichern und testen" als billiger
    /// Verbindungscheck (Story AI-A4).
    fn list_models(&self) -> Result<Vec<String>, AiError>;

    /// Chat-Aufruf mit JSON-Erzwingung im Prompt und Validierung der Antwort.
    /// Bei unparsbarer Ausgabe: ein stiller Retry mit Format-Erinnerung,
    /// danach [`AiError::InvalidJson`]. Transportfehler werden nicht erneut
    /// versucht — der Retry gilt nur dem Ausgabeformat.
    fn complete_json(&self, messages: &[ChatMessage]) -> Result<serde_json::Value, AiError> {
        let effective = with_json_instruction(messages);
        let first = self.chat(&effective)?;
        match extract_json_object(&first) {
            Ok(value) => Ok(value),
            Err(_) => {
                // Stiller Retry: unbrauchbare Antwort plus Erinnerung anhängen.
                let mut retry = effective;
                retry.push(ChatMessage::assistant(&first));
                retry.push(ChatMessage::user(JSON_RETRY_REMINDER));
                let second = self.chat(&retry)?;
                extract_json_object(&second)
            }
        }
    }
}

/// Blockierender Client gegen einen OpenAI-kompatiblen Endpunkt.
/// Basis-URL inklusive `/v1` (z. B. `http://localhost:11434/v1`), wie von
/// den Provider-Presets in den Einstellungen vorbefüllt.
pub struct LlmClient {
    http: reqwest::blocking::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl LlmClient {
    pub fn new(base_url: &str, model: &str, api_key: Option<String>) -> Result<Self, AiError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| AiError::Config(e.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            // Leerer Key wie „kein Key" behandeln — kein leerer Bearer-Header.
            api_key: api_key.filter(|k| !k.is_empty()),
        })
    }

    /// Client aus den Einstellungen (AI-A1): Basis-URL und Modell aus der
    /// Config, API-Key aus dem Secret Service (nur für Cloud-Endpunkte nötig).
    pub fn from_settings(settings: &crate::config::Settings) -> Result<Self, AiError> {
        let api_key = crate::secrets::get_ai_api_key().map_err(AiError::Config)?;
        Self::new(&settings.ai_base_url, &settings.ai_model, api_key)
    }

    /// Hängt den Pfad an die normalisierte Basis-URL an.
    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// Setzt den Bearer-Header, falls ein API-Key konfiguriert ist.
    fn authorized(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        match &self.api_key {
            Some(key) => req.bearer_auth(key),
            None => req,
        }
    }

    /// Führt den Request aus und liefert den Body als Text; Fehlerstatus wird
    /// mit einem Body-Auszug gemeldet (Bodies können riesig sein).
    fn send(&self, req: reqwest::blocking::RequestBuilder) -> Result<String, AiError> {
        let resp = req.send().map_err(|e| AiError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| AiError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(AiError::Api(format!("HTTP {status}: {}", excerpt(&text))));
        }
        Ok(text)
    }
}

impl Llm for LlmClient {
    fn chat(&self, messages: &[ChatMessage]) -> Result<String, AiError> {
        // Erste Stufe der JSON-Erzwingung: `response_format` im Request.
        // Ollama und die meisten OpenRouter-Modelle honorieren es; wo nicht,
        // greifen Prompt-Anweisung und Validierung in `complete_json`.
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "response_format": {"type": "json_object"},
            "stream": false,
        });
        let req = self.authorized(self.http.post(self.endpoint("/chat/completions")).json(&body));
        parse_chat_response(&self.send(req)?)
    }

    fn list_models(&self) -> Result<Vec<String>, AiError> {
        let req = self.authorized(self.http.get(self.endpoint("/models")));
        parse_models_response(&self.send(req)?)
    }
}

/// Legt die JSON-Anweisung in die System-Nachricht: vorhandene System-Nachricht
/// wird ergänzt, sonst wird eine vorangestellt (zweite Stufe der Erzwingung).
fn with_json_instruction(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut out = messages.to_vec();
    match out.first_mut() {
        Some(first) if first.role == "system" => {
            first.content = format!("{}\n\n{JSON_INSTRUCTION}", first.content);
        }
        _ => out.insert(0, ChatMessage::system(JSON_INSTRUCTION)),
    }
    out
}

/// Validiert Modell-Ausgabe als einzelnes JSON-Objekt. Markdown-Code-Zäune
/// werden vorher entfernt — Modelle, deren Endpunkt `response_format`
/// ignoriert, verpacken JSON gern in ```json-Blöcke.
fn extract_json_object(text: &str) -> Result<serde_json::Value, AiError> {
    let cleaned = strip_code_fences(text);
    let value: serde_json::Value = serde_json::from_str(cleaned)
        .map_err(|_| AiError::InvalidJson(excerpt(text)))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(AiError::InvalidJson(excerpt(text)))
    }
}

/// Entfernt einen umschließenden Markdown-Code-Zaun (z. B. ```json … ```).
fn strip_code_fences(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // Erste Zeile (Zaun samt Sprach-Tag) verwerfen; ohne Zeilenumbruch ist es
    // kein Block, dann lieber unverändert an den JSON-Parser geben.
    let Some(newline) = rest.find('\n') else {
        return trimmed;
    };
    let body = rest[newline + 1..].trim_end();
    body.strip_suffix("```").unwrap_or(body).trim()
}

// ─── Antwort-Parsing (Ollama- und OpenRouter-Shapes) ────────────────────────

/// Fehlerobjekt im Body — OpenRouter liefert Fehler teils mit HTTP 200.
#[derive(Deserialize)]
struct ApiErrorBody {
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
    error: Option<ApiErrorBody>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

/// Zieht den Antworttext aus einer Chat-Completion-Antwort. Toleriert die
/// Zusatzfelder der Provider (Ollama: `system_fingerprint` …; OpenRouter:
/// `provider`, `usage`, `reasoning` …), kennt aber beide Fehlerformen.
fn parse_chat_response(body: &str) -> Result<String, AiError> {
    let parsed: ChatCompletionResponse = serde_json::from_str(body)
        .map_err(|_| AiError::Api(format!("Unerwartete Antwortform: {}", excerpt(body))))?;
    if let Some(err) = parsed.error {
        return Err(AiError::Api(err.message));
    }
    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| AiError::Api("Antwort ohne Inhalt".into()))
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
    error: Option<ApiErrorBody>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Zieht die Modellnamen aus einer `/v1/models`-Antwort (OpenAI-Listenform,
/// bei Ollama wie OpenRouter: `{"data": [{"id": …}, …]}`).
fn parse_models_response(body: &str) -> Result<Vec<String>, AiError> {
    let parsed: ModelsResponse = serde_json::from_str(body)
        .map_err(|_| AiError::Api(format!("Unerwartete Antwortform: {}", excerpt(body))))?;
    if let Some(err) = parsed.error {
        return Err(AiError::Api(err.message));
    }
    Ok(parsed.data.into_iter().map(|m| m.id).collect())
}

/// Kürzt Texte für Fehlermeldungen; zeichenweise, damit Umlaute keine
/// UTF-8-Grenzen verletzen.
fn excerpt(text: &str) -> String {
    const MAX: usize = 200;
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX {
        trimmed.to_string()
    } else {
        let kurz: String = trimmed.chars().take(MAX).collect();
        format!("{kurz}…")
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Mock für Tests der App-Logik ohne Netz: liefert vorbereitete Antworten
    /// in Reihenfolge und protokolliert jeden Aufruf samt Nachrichten.
    pub struct MockLlm {
        antworten: RefCell<VecDeque<Result<String, AiError>>>,
        pub aufrufe: RefCell<Vec<Vec<ChatMessage>>>,
        modelle: Vec<String>,
    }

    impl MockLlm {
        pub fn new(antworten: Vec<Result<String, AiError>>) -> Self {
            Self {
                antworten: RefCell::new(antworten.into()),
                aufrufe: RefCell::new(Vec::new()),
                modelle: vec!["mock-modell".into()],
            }
        }
    }

    impl Llm for MockLlm {
        fn chat(&self, messages: &[ChatMessage]) -> Result<String, AiError> {
            self.aufrufe.borrow_mut().push(messages.to_vec());
            self.antworten
                .borrow_mut()
                .pop_front()
                .expect("MockLlm: mehr Aufrufe als vorbereitete Antworten")
        }

        fn list_models(&self) -> Result<Vec<String>, AiError> {
            Ok(self.modelle.clone())
        }
    }

    // ─── Wire-Format und Prompt-Aufbau ──────────────────────────────────────

    #[test]
    fn chat_message_serialisiert_ins_openai_wire_format() {
        let msg = ChatMessage::user("hallo");
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            serde_json::json!({"role": "user", "content": "hallo"})
        );
    }

    #[test]
    fn json_instruction_ergaenzt_vorhandene_system_nachricht() {
        let eingabe = vec![
            ChatMessage::system("Du bist ein Task-Parser."),
            ChatMessage::user("Milch kaufen morgen"),
        ];
        let effektiv = with_json_instruction(&eingabe);
        assert_eq!(effektiv.len(), 2);
        assert_eq!(effektiv[0].role, "system");
        assert!(effektiv[0].content.starts_with("Du bist ein Task-Parser."));
        assert!(effektiv[0].content.contains(JSON_INSTRUCTION));
        // Ursprüngliche Nachrichten bleiben unverändert.
        assert_eq!(eingabe[0].content, "Du bist ein Task-Parser.");
    }

    #[test]
    fn json_instruction_stellt_system_nachricht_voran_wenn_keine_da() {
        let effektiv = with_json_instruction(&[ChatMessage::user("Milch kaufen")]);
        assert_eq!(effektiv.len(), 2);
        assert_eq!(effektiv[0], ChatMessage::system(JSON_INSTRUCTION));
        assert_eq!(effektiv[1].content, "Milch kaufen");
    }

    // ─── JSON-Validierung ───────────────────────────────────────────────────

    #[test]
    fn extract_json_akzeptiert_objekt() {
        let v = extract_json_object(r#"{"title": "Milch kaufen", "due": "morgen"}"#).unwrap();
        assert_eq!(v["title"], "Milch kaufen");
    }

    #[test]
    fn extract_json_lehnt_kaputtes_json_ab() {
        let err = extract_json_object(r#"{"title": "Milch"#).unwrap_err();
        assert!(matches!(err, AiError::InvalidJson(_)));
    }

    #[test]
    fn extract_json_lehnt_nicht_objekte_ab() {
        // Gültiges JSON, aber kein Objekt — `json_object` verlangt ein Objekt.
        assert!(matches!(
            extract_json_object(r#"["a", "b"]"#),
            Err(AiError::InvalidJson(_))
        ));
        assert!(matches!(
            extract_json_object("Hier ist dein Task!"),
            Err(AiError::InvalidJson(_))
        ));
    }

    #[test]
    fn extract_json_entfernt_markdown_zaeune() {
        let text = "```json\n{\"title\": \"Milch kaufen\"}\n```";
        let v = extract_json_object(text).unwrap();
        assert_eq!(v["title"], "Milch kaufen");
        // Auch ohne Sprach-Tag.
        let v = extract_json_object("```\n{\"a\": 1}\n```").unwrap();
        assert_eq!(v["a"], 1);
    }

    // ─── Retry-Pfad ─────────────────────────────────────────────────────────

    #[test]
    fn complete_json_ohne_retry_bei_gueltiger_antwort() {
        let mock = MockLlm::new(vec![Ok(r#"{"title": "Milch"}"#.into())]);
        let v = mock.complete_json(&[ChatMessage::user("Milch")]).unwrap();
        assert_eq!(v["title"], "Milch");
        assert_eq!(mock.aufrufe.borrow().len(), 1);
    }

    #[test]
    fn complete_json_retry_mit_format_erinnerung() {
        let mock = MockLlm::new(vec![
            Ok("Klar! Hier ist dein Task als JSON: {…}".into()),
            Ok(r#"{"title": "Milch"}"#.into()),
        ]);
        let v = mock.complete_json(&[ChatMessage::user("Milch")]).unwrap();
        assert_eq!(v["title"], "Milch");

        let aufrufe = mock.aufrufe.borrow();
        assert_eq!(aufrufe.len(), 2);
        // Der Retry trägt die unbrauchbare Antwort und die Format-Erinnerung.
        let retry = &aufrufe[1];
        assert_eq!(retry[retry.len() - 2].role, "assistant");
        assert!(retry[retry.len() - 2].content.contains("Klar!"));
        assert_eq!(retry.last().unwrap(), &ChatMessage::user(JSON_RETRY_REMINDER));
    }

    #[test]
    fn complete_json_fehler_nach_zweiter_kaputter_antwort() {
        let mock = MockLlm::new(vec![
            Ok("kein json".into()),
            Ok("immer noch kein json".into()),
        ]);
        let err = mock.complete_json(&[ChatMessage::user("Milch")]).unwrap_err();
        assert!(matches!(err, AiError::InvalidJson(_)));
        assert_eq!(mock.aufrufe.borrow().len(), 2);
    }

    #[test]
    fn complete_json_kein_retry_bei_transportfehler() {
        // Der Retry gilt nur dem Ausgabeformat — Netzwerkfehler sofort melden.
        let mock = MockLlm::new(vec![Err(AiError::Network("connection refused".into()))]);
        let err = mock.complete_json(&[ChatMessage::user("Milch")]).unwrap_err();
        assert!(matches!(err, AiError::Network(_)));
        assert_eq!(mock.aufrufe.borrow().len(), 1);
    }

    // ─── Antwort-Shapes: Ollama und OpenRouter ──────────────────────────────

    #[test]
    fn parse_chat_response_ollama_shape() {
        // Ollama 0.3x, /v1/chat/completions (OpenAI-Kompatibilitätsschicht).
        let body = r#"{
            "id": "chatcmpl-620",
            "object": "chat.completion",
            "created": 1753948800,
            "model": "granite4.1:8b",
            "system_fingerprint": "fp_ollama",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "{\"title\": \"Milch kaufen\"}"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 25, "completion_tokens": 9, "total_tokens": 34}
        }"#;
        assert_eq!(parse_chat_response(body).unwrap(), r#"{"title": "Milch kaufen"}"#);
    }

    #[test]
    fn parse_chat_response_openrouter_shape() {
        // OpenRouter: zusätzliche Felder (provider, reasoning, native_finish_reason).
        let body = r#"{
            "id": "gen-1753948800-abc",
            "provider": "Anthropic",
            "model": "anthropic/claude-3.5-haiku",
            "object": "chat.completion",
            "created": 1753948800,
            "choices": [{
                "logprobs": null,
                "finish_reason": "stop",
                "native_finish_reason": "end_turn",
                "index": 0,
                "message": {"role": "assistant", "content": "{\"title\": \"Milch kaufen\"}", "refusal": null, "reasoning": null}
            }],
            "usage": {"prompt_tokens": 25, "completion_tokens": 9, "total_tokens": 34}
        }"#;
        assert_eq!(parse_chat_response(body).unwrap(), r#"{"title": "Milch kaufen"}"#);
    }

    #[test]
    fn parse_chat_response_fehlerobjekt_trotz_http_200() {
        // OpenRouter meldet manche Fehler als Body-Objekt mit HTTP 200.
        let body = r#"{"error": {"message": "Rate limit exceeded", "code": 429}}"#;
        let err = parse_chat_response(body).unwrap_err();
        assert_eq!(err, AiError::Api("Rate limit exceeded".into()));
    }

    #[test]
    fn parse_chat_response_ohne_inhalt_ist_fehler() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": ""}}]}"#;
        assert!(matches!(parse_chat_response(body), Err(AiError::Api(_))));
        assert!(matches!(parse_chat_response("kein json"), Err(AiError::Api(_))));
    }

    #[test]
    fn parse_models_ollama_shape() {
        let body = r#"{
            "object": "list",
            "data": [
                {"id": "granite4.1:8b", "object": "model", "created": 1753948800, "owned_by": "library"},
                {"id": "qwen3.6:27b-128k", "object": "model", "created": 1753948800, "owned_by": "library"}
            ]
        }"#;
        assert_eq!(
            parse_models_response(body).unwrap(),
            vec!["granite4.1:8b".to_string(), "qwen3.6:27b-128k".to_string()]
        );
    }

    #[test]
    fn parse_models_openrouter_shape() {
        // OpenRouter: reichhaltige Einträge — nur `id` interessiert.
        let body = r#"{
            "data": [{
                "id": "anthropic/claude-3.5-haiku",
                "name": "Anthropic: Claude 3.5 Haiku",
                "created": 1753948800,
                "description": "…",
                "context_length": 200000,
                "architecture": {"modality": "text->text", "tokenizer": "Claude"},
                "pricing": {"prompt": "0.0000008", "completion": "0.000004"},
                "top_provider": {"context_length": 200000, "is_moderated": true}
            }]
        }"#;
        assert_eq!(
            parse_models_response(body).unwrap(),
            vec!["anthropic/claude-3.5-haiku".to_string()]
        );
    }

    #[test]
    fn base_url_mit_und_ohne_schlussstrich() {
        let mit = LlmClient::new("http://localhost:11434/v1/", "m", None).unwrap();
        let ohne = LlmClient::new("http://localhost:11434/v1", "m", None).unwrap();
        assert_eq!(mit.endpoint("/models"), "http://localhost:11434/v1/models");
        assert_eq!(ohne.endpoint("/models"), "http://localhost:11434/v1/models");
    }

    #[test]
    fn leerer_api_key_wird_wie_kein_key_behandelt() {
        let client = LlmClient::new("http://localhost:11434/v1", "m", Some(String::new())).unwrap();
        assert_eq!(client.api_key, None);
    }

    #[test]
    fn send_meldet_fehlerstatus_mit_body_auszug() {
        // Nicht-2xx-Pfad von `send` gegen einen Mini-HTTP-Server aus der
        // Standardbibliothek: die Meldung trägt Status UND Body-Auszug.
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let adresse = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut verbindung, _) = listener.accept().unwrap();
            // Request grob einlesen (Inhalt egal), dann 500 mit Body liefern.
            let mut puffer = [0u8; 4096];
            let _ = verbindung.read(&mut puffer);
            let body = r#"{"error": {"message": "kaputt"}}"#;
            let antwort = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            verbindung.write_all(antwort.as_bytes()).unwrap();
        });
        let client = LlmClient::new(&format!("http://{adresse}/v1"), "m", None).unwrap();
        let fehler = client.chat(&[ChatMessage::user("hi")]).unwrap_err();
        server.join().unwrap();
        match fehler {
            AiError::Api(meldung) => {
                assert!(meldung.contains("500"), "Status fehlt: {meldung}");
                assert!(meldung.contains("kaputt"), "Body-Auszug fehlt: {meldung}");
            }
            andere => panic!("Api-Fehler erwartet, war: {andere:?}"),
        }
    }

    /// Echter Roundtrip gegen ein lokales Ollama — braucht den laufenden
    /// Dienst und mindestens ein Chat-Modell, daher `#[ignore]`:
    ///
    ///     cargo test -p vergissmeinnicht-app -- --ignored ollama
    #[test]
    #[ignore]
    fn live_ollama_roundtrip() {
        let sonde = LlmClient::new("http://localhost:11434/v1", "", None).unwrap();
        let modelle = sonde.list_models().expect("Modellliste");
        assert!(!modelle.is_empty(), "kein Modell installiert");
        // Embedding-Modelle können nicht chatten — erstes Chat-Modell nehmen.
        let modell = modelle
            .iter()
            .find(|m| !m.contains("embed"))
            .expect("kein Chat-Modell installiert");
        let client = LlmClient::new("http://localhost:11434/v1", modell, None).unwrap();
        let antwort = client
            .complete_json(&[ChatMessage::user(
                "Gib ein JSON-Objekt mit dem Feld \"gruss\" und dem Wert \"hallo\" zurück.",
            )])
            .expect("Completion");
        assert!(antwort.is_object());
    }
}
