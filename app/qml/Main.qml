import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import de.hnsstrk.vergissmeinnicht

Kirigami.ApplicationWindow {
    id: root

    title: "Vergissmeinnicht"
    width: 1200
    height: 760
    minimumWidth: 760
    minimumHeight: 480

    // Geparste Sidebar-Daten (JSON-Properties der Bridge).
    property var counts: ({})
    property var projects: []
    property var tagList: []
    property var savedSearches: []
    // Aktive Drag-Auswahl (UUIDs) für Drag & Drop auf die Sidebar.
    property var dragUuids: []

    function reparse() {
        counts = JSON.parse(app.countsJson || "{}")
        projects = JSON.parse(app.projectsJson || "[]")
        tagList = JSON.parse(app.tagsJson || "[]")
        savedSearches = JSON.parse(app.savedSearchesJson || "[]")
    }

    function filterTitle(key) {
        if (key.startsWith("project:"))
            return key.substring(8)
        if (key.startsWith("tag:"))
            return "#" + key.substring(4)
        if (key.startsWith("saved:")) {
            const id = key.substring(6)
            const hit = savedSearches.find(s => s.id === id)
            return hit ? hit.name : i18n("Gespeicherte Suche")
        }
        switch (key) {
        case "all": return i18n("Alle")
        case "today": return i18n("Heute")
        case "todo": return i18n("Zu erledigen")
        case "overdue": return i18n("Überfällig")
        case "duesoon": return i18n("Bald fällig")
        case "upcoming": return i18n("Geplant")
        case "waiting": return i18n("Wartend")
        case "active": return i18n("Aktiv")
        case "blocked": return i18n("Blockiert")
        case "blocking": return i18n("Blockierend")
        case "unblocked": return i18n("Nicht blockiert")
        default: return i18n("Eingang")
        }
    }

    function openDetail(uuid) {
        detailDialog.openFor(uuid)
    }

    AppContainer {
        id: app
        onCountsJsonChanged: root.reparse()
        onProjectsJsonChanged: root.reparse()
        onTagsJsonChanged: root.reparse()
        onSavedSearchesJsonChanged: root.reparse()
    }

    Component.onCompleted: {
        reparse()
        // Startverhalten wie macOS: Sync falls konfiguriert (fällt sonst auf
        // Aktualisieren zurück), danach ggf. Überfällig-Benachrichtigung.
        app.startSync()
        app.maybeNotifyOverdue()

        // Testhaken für Screenshots/Verifikation: --test-dialog=<name> öffnet
        // den jeweiligen Dialog direkt nach dem Start; --test-grab=<datei>
        // rendert das Fenster nach 3 s in eine PNG-Datei und beendet die App
        // (funktioniert auch, wenn der Compositor keine Frames liefert).
        for (const arg of Qt.application.arguments) {
            if (arg.startsWith("--test-dialog=")) {
                testDialogTimer.dialogName = arg.substring(14)
                testDialogTimer.start()
            } else if (arg.startsWith("--test-grab=")) {
                testGrabTimer.path = arg.substring(12)
                testGrabTimer.start()
            } else if (arg === "--test-flow") {
                testFlowTimer.start()
            } else if (arg === "--test-secrets") {
                testSecretsTimer.start()
            } else if (arg === "--test-settings-ui") {
                testSettingsUiTimer.start()
            }
        }
    }

    // UI-Test des Einstellungsdialogs mit echten Klicks/Tastatur: Felder
    // anklicken, Werte tippen, „Speichern und Sync testen“ klicken, Persistenz
    // und Wiederöffnen prüfen. Braucht einen lokalen Sync-Server auf :18080.
    Timer {
        id: testSettingsUiTimer
        property int step: 0
        property int failures: 0
        readonly property string url: "http://127.0.0.1:18080"
        readonly property string cid: "550e8400-e29b-41d4-a716-446655440000"
        readonly property string secret: "ui-test-geheimnis"
        interval: 700
        repeat: true

        function check(cond, label) {
            console.log((cond ? "SETTINGS-OK  " : "SETTINGS-FAIL ") + label
                        + (app.errorMessage.length > 0 ? "  [Fehler: " + app.errorMessage + "]" : ""))
            if (!cond) failures++
        }
        function typeText(text) {
            for (const ch of text) {
                const key = ch === " " ? Qt.Key_Space : ch.toUpperCase().charCodeAt(0)
                app.testKey(key, ch === ch.toUpperCase() && ch !== ch.toLowerCase()
                            ? Qt.ShiftModifier : 0, ch)
            }
        }

        onTriggered: {
            step++
            switch (step) {
            case 1:
                // Direkt auf der Sync-Seite öffnen (Kategorien seit UI-4) und
                // das Fenster hoch ziehen: alle Formularzeilen im fertigen
                // Layout, damit die synthetischen Klicks treffen.
                settingsDialog.openSettings("sync")
                settingsDialog.configViewItem.height = 900
                break
            case 2: {
                // Erzwingt einen synchronen Render: das frisch geöffnete
                // ApplicationWindow layoutet asynchron, sonst treffen die
                // ersten Klicks ein noch unfertiges Layout (Race).
                app.grabWindowTo("/tmp/settings-ui-layout-sync.png")
                const p = settingsDialog.testPoints().url
                app.testClick(p.x, p.y, Qt.LeftButton, 0, false)
                typeText(url)
                break
            }
            case 3: {
                const p = settingsDialog.testPoints().clientId
                app.testClick(p.x, p.y, Qt.LeftButton, 0, false)
                typeText(cid)
                break
            }
            case 4: {
                const p = settingsDialog.testPoints().secret
                app.testClick(p.x, p.y, Qt.LeftButton, 0, false)
                typeText(secret)
                const v = settingsDialog.testValues()
                check(v.url === url && v.clientId === cid && v.secret === secret,
                      "Getippte Werte stehen in den Feldern")
                break
            }
            case 5: {
                const p = settingsDialog.testPoints().save
                app.testClick(p.x, p.y, Qt.LeftButton, 0, false)
                break
            }
            case 6:
            case 7:
            case 8:
            case 9:
                // Sync-Abschluss abwarten.
                if (app.isSyncing)
                    return
                step = 9
                break
            case 10: {
                check(app.syncClientId() === cid, "Client-ID persistiert (Secret Service)")
                check(app.syncSecret() === secret, "Secret persistiert (Secret Service)")
                check(app.syncServerUrl === url, "Server-URL persistiert (Config)")
                check(app.syncConfigured, "syncConfigured true")
                check(app.errorMessage.length === 0, "kein Fehler")
                check(app.lastSyncAt > 0, "Test-Sync erfolgreich")
                app.grabWindowTo("/tmp/settings-ui-after-save.png")
                settingsDialog.close()
                break
            }
            case 11:
                // Wiederöffnen: Felder müssen die gespeicherten Werte zeigen.
                settingsDialog.openSettings("sync")
                break
            case 12: {
                const v = settingsDialog.testValues()
                check(v.url === url && v.clientId === cid && v.secret === secret,
                      "Wiederöffnen lädt gespeicherte Werte")
                // Aufräumen.
                app.setSyncCredentials("", "")
                app.setSyncServerUrlSetting("")
                console.log(failures === 0
                            ? "SETTINGS-ENDE: alles grün"
                            : `SETTINGS-ENDE: ${failures} Fehler`)
                testSettingsUiTimer.running = false
                Qt.quit()
                break
            }
            }
        }
    }

    // Repliziert exakt die „Speichern und Sync testen“-Sequenz des
    // Einstellungsdialogs und protokolliert jeden Schritt (SECRETS-…).
    Timer {
        id: testSecretsTimer
        interval: 1500
        onTriggered: {
            let failures = 0
            function check(cond, label) {
                console.log((cond ? "SECRETS-OK  " : "SECRETS-FAIL ") + label
                            + (app.errorMessage.length > 0 ? "  [Fehler: " + app.errorMessage + "]" : ""))
                if (!cond) failures++
            }
            const url = "http://127.0.0.1:18080"
            const cid = "550e8400-e29b-41d4-a716-446655440000"
            const secret = "test-geheimnis-123"

            app.setSyncServerUrlSetting(url)
            check(app.syncServerUrl === url, "Server-URL gesetzt")

            const credsOk = app.setSyncCredentials(cid, secret)
            check(credsOk, "setSyncCredentials meldet Erfolg")
            check(app.syncClientId() === cid, "Client-ID zurückgelesen")
            check(app.syncSecret() === secret, "Secret zurückgelesen")
            check(app.syncConfigured, "syncConfigured ist true")

            app.startSync()
            console.log("SECRETS-INFO startSync ausgelöst, isSyncing=" + app.isSyncing)
            syncWaiter.start()
        }
    }
    Timer {
        id: syncWaiter
        interval: 8000
        onTriggered: {
            console.log("SECRETS-INFO nach Sync: isSyncing=" + app.isSyncing
                        + " lastSyncAt=" + app.lastSyncAt
                        + " Fehler=" + (app.errorMessage.length > 0 ? app.errorMessage : "(keiner)"))
            // Aufräumen: Test-Credentials wieder entfernen.
            app.setSyncCredentials("", "")
            app.setSyncServerUrlSetting("")
            console.log("SECRETS-ENDE")
            Qt.quit()
        }
    }

    // Funktions-Smoke-Test über die echte QML→Bridge-Kette (siehe CLAUDE.md).
    // Läuft gegen die aktive Replica — nur mit Wegwerf-Daten (XDG_DATA_HOME)
    // verwenden. Ausgabe: FLOW-OK/FLOW-FAIL-Zeilen auf der Konsole.
    Timer {
        id: testFlowTimer
        interval: 1500
        onTriggered: {
            let failures = 0
            function check(cond, label) {
                console.log((cond ? "FLOW-OK  " : "FLOW-FAIL ") + label)
                if (!cond) failures++
            }
            function uuids() { return Array.from(app.visibleUuids(0, 9999)) }
            function taskOf(u) { return JSON.parse(app.taskJson(u)) }

            // 1. Quick Capture mit Token-Syntax
            check(app.quickCaptureCommit("Flow-Testaufgabe +flowtest project:flowdemo due:tomorrow priority:H"),
                  "quickCaptureCommit")
            app.applyFilter("project:flowdemo")
            check(uuids().length === 1, "Projektfilter zeigt neue Aufgabe")
            const u1 = uuids()[0]
            let t = taskOf(u1)
            check(t.project === "flowdemo" && t.priority === "H"
                  && t.tags.indexOf("flowtest") !== -1 && t.due > 0,
                  "Token-Metadaten persistiert")

            // 2. Suche mit Operatoren
            app.applySearch("tag:flowtest status:offen")
            check(uuids().length === 1, "Suche tag:+status: findet Aufgabe")
            app.applySearch("")

            // 3. Detail-Speichern (atomar + Einzel-Setter)
            const morgen = app.parseDueToken("tomorrow")
            check(app.saveTaskDetail(u1, "Flow-Test umbenannt", "flowdemo", "flowtest",
                                     morgen, 0, 0, "M", "weekly", 0),
                  "saveTaskDetail")
            t = taskOf(u1)
            check(t.description === "Flow-Test umbenannt" && t.priority === "M"
                  && t.recur === "weekly" && t.due === morgen,
                  "Detail-Änderungen persistiert")

            // 4. Notizen
            app.addTaskAnnotation(u1, "Flow-Notiz")
            t = taskOf(u1)
            check(t.annotations.length === 1 && t.annotations[0].description === "Flow-Notiz",
                  "Annotation angelegt")
            app.removeTaskAnnotation(u1, t.annotations[0].entry)
            check(taskOf(u1).annotations.length === 0, "Annotation entfernt")

            // 5. Snooze
            app.snoozeTask(u1, Math.floor(Date.now() / 1000) + 86400)
            check(taskOf(u1).wait > 0, "Snooze gesetzt")
            app.snoozeTask(u1, 0)
            check(taskOf(u1).wait === null, "Snooze aufgehoben")

            // 5b. Start/Stop, Undo, Duplizieren, until, Urgency, virtuelle Tags
            app.startTask(u1)
            check(taskOf(u1).start > 0, "startTask setzt start")
            app.applySearch("+ACTIVE")
            check(uuids().indexOf(u1) !== -1, "+ACTIVE findet aktive Aufgabe")
            app.applySearch("")
            app.stopTask(u1)
            check(taskOf(u1).start === null, "stopTask entfernt start")
            app.bulkAddTag([u1], "undotest")
            check(taskOf(u1).tags.indexOf("undotest") !== -1, "Tag für Undo-Test gesetzt")
            app.undoLastChange()
            check(taskOf(u1).tags.indexOf("undotest") === -1, "undo entfernt letzte Änderung")
            check(app.saveTaskDetail(u1, taskOf(u1).description, "flowdemo", "flowtest",
                                     morgen, 0, 0, "M", "weekly", morgen + 86400 * 30),
                  "saveTaskDetail mit until")
            check(taskOf(u1).until === morgen + 86400 * 30, "until persistiert")
            check(taskOf(u1).urgency !== undefined && taskOf(u1).urgency > 0, "urgency berechnet")
            app.duplicateTask(u1)
            app.applyFilter("project:flowdemo")
            check(uuids().filter(u => taskOf(u).description === taskOf(u1).description).length === 2,
                  "duplicateTask erzeugt Kopie")
            const kopie = uuids().find(u => u !== u1 && taskOf(u).description === taskOf(u1).description)
            app.deleteTasks([kopie])
            app.setSort("urgency", true)
            check(app.sortKey === "urgency", "Urgency-Sortierung aktiv")
            app.setSort("id", true)

            // 6. Recurring: Erledigen erzeugt Folge-Instanz
            app.applyFilter("project:flowdemo")
            app.markDone(u1)
            const nach = uuids().map(taskOf)
            const folge = nach.find(x => x.status === "pending" && x.uuid !== u1)
            check(taskOf(u1).status === "completed", "Original erledigt")
            check(!!folge && folge.recur === "weekly" && folge.due > morgen,
                  "Folge-Instanz mit verschobener Fälligkeit")

            // 7. Bulk-Aktionen
            app.bulkAddTag([folge.uuid], "flowbulk")
            check(taskOf(folge.uuid).tags.indexOf("flowbulk") !== -1, "bulkAddTag")
            app.bulkSetPriority([folge.uuid], "L")
            check(taskOf(folge.uuid).priority === "L", "bulkSetPriority")
            app.bulkSetDue([folge.uuid], 0)
            check(taskOf(folge.uuid).due === null, "bulkSetDue leeren")

            // 8. Gespeicherte Suche
            app.applySearch("tag:flowtest")
            check(app.saveCurrentSearch("FlowSuche"), "saveCurrentSearch")
            const gespeichert = JSON.parse(app.savedSearchesJson)
            check(gespeichert.some(s => s.name === "FlowSuche"), "Suche in savedSearchesJson")
            const sid = gespeichert.find(s => s.name === "FlowSuche").id
            app.applyFilter("saved:" + sid)
            check(app.searchQuery === "tag:flowtest", "Saved Search aktiviert Query")
            app.deleteSavedSearch(sid)
            check(!JSON.parse(app.savedSearchesJson).some(s => s.name === "FlowSuche"),
                  "Saved Search gelöscht")

            // 8b. Abhängigkeiten (Editor-Pfad)
            check(app.quickCaptureCommit("Flow-Blocker +flowtest"), "Blocker angelegt")
            app.applySearch("tag:flowtest status:offen")
            const beide = uuids().map(taskOf)
            const blockerTask = beide.find(x => x.description === "Flow-Blocker")
            app.addTaskDependency(folge.uuid, blockerTask.uuid)
            check(taskOf(folge.uuid).depends.indexOf(blockerTask.uuid) !== -1
                  && taskOf(folge.uuid).isBlocked && taskOf(blockerTask.uuid).isBlocking,
                  "Abhängigkeit gesetzt (blocked/blocking)")
            check(JSON.parse(app.pendingTasksJson()).some(t => t.uuid === blockerTask.uuid),
                  "pendingTasksJson enthält Blocker")
            app.removeTaskDependency(folge.uuid, blockerTask.uuid)
            check(taskOf(folge.uuid).depends.length === 0 && !taskOf(folge.uuid).isBlocked,
                  "Abhängigkeit entfernt")
            app.applySearch("")

            // 8c. Legacy-Reparatur: Tokens im Titel → Properties
            check(app.addTaskDetailed("Legacy-Aufgabe +flowtest project:flowlegacy priority:H", "", "", 0, "", "", ""),
                  "Legacy-Aufgabe angelegt")
            const repariert = app.repairLegacyTasks()
            check(repariert >= 1, "repairLegacyTasks meldet Reparatur")
            app.applyFilter("project:flowlegacy")
            const legacy = uuids().map(taskOf).find(x => x.description === "Legacy-Aufgabe")
            check(!!legacy && legacy.project === "flowlegacy"
                  && legacy.tags.indexOf("flowtest") !== -1 && legacy.priority === "H",
                  "Legacy-Tokens in Properties überführt")

            // 8d. Custom-Werte (AI-B0): freie due/recur-Tokens wie im
            // Quick-Capture-Dialog bei „Benutzerdefiniert …" committet.
            const customDue = app.parseDueToken("+3d")
            check(customDue > 0, "parseDueToken versteht +3d")
            check(app.isValidRecurToken("quarterly"), "isValidRecurToken versteht quarterly")
            check(app.addTaskDetailed("Flow-Custom", "", "flowtest", customDue, "", "quarterly", ""),
                  "addTaskDetailed mit Custom-due/-recur")
            app.applyFilter("all")
            const custom = uuids().map(taskOf).find(x => x.description === "Flow-Custom")
            check(!!custom && custom.due === customDue && custom.recur === "quarterly",
                  "Custom-Werte persistiert")

            // 9. Tag/Projekt-Management
            app.renameTag("flowtest", "flowfertig")
            check(taskOf(folge.uuid).tags.indexOf("flowfertig") !== -1
                  && taskOf(folge.uuid).tags.indexOf("flowtest") === -1, "renameTag")
            app.renameProject("flowdemo", "flowdemo2")
            check(taskOf(folge.uuid).project === "flowdemo2", "renameProject")

            // 10. Sidebar-Sektionen: Toggle + Persistenz
            root.globalDrawer.toggleSection("tags")
            check(app.collapsedSectionsJson === '["tags"]', "Sektion eingeklappt persistiert")
            root.globalDrawer.toggleSection("tags")
            check(app.collapsedSectionsJson === "[]", "Sektion wieder ausgeklappt")

            // 11. Aufräumen: alle Flow-Aufgaben löschen
            app.applyFilter("all")
            const opfer = uuids().filter(u => {
                const x = taskOf(u)
                return x.project === "flowdemo2" || (x.tags ?? []).indexOf("flowfertig") !== -1
            })
            app.deleteTasks(opfer)
            app.applyFilter("all")
            check(!uuids().some(u => taskOf(u).project === "flowdemo2"), "Aufräumen")

            // 12. Sync-Aktion nur bei konfiguriertem Sync-Server sichtbar/aktiv (UI-2).
            // Ursprünglichen Zustand merken, um ihn danach wiederherzustellen.
            const syncUrlZuvor = app.syncServerUrl
            const syncCidZuvor = app.syncClientId()
            const syncSecretZuvor = app.syncSecret()

            app.setSyncServerUrlSetting("")
            check(!app.syncConfigured, "syncConfigured false ohne Server-URL")
            check(!tasksPage.syncAction.visible, "Sync-Aktion ausgeblendet ohne Konfiguration")
            check(!tasksPage.syncAction.enabled, "Sync-Aktion deaktiviert ohne Konfiguration")

            // Der Positivfall braucht den Secret Service (Client-ID + Secret);
            // in Umgebungen ohne den Dienst (CI-Container) wird er wie der
            // KI-Worker-Teil übersprungen statt rot zu laufen.
            const syncCredsOk = app.setSyncCredentials("550e8400-e29b-41d4-a716-446655440000",
                                                       "flow-test-geheimnis")
            if (syncCredsOk) {
                app.setSyncServerUrlSetting("http://127.0.0.1:18080")
                check(app.syncConfigured, "syncConfigured true mit vollständiger Konfiguration")
                check(tasksPage.syncAction.visible, "Sync-Aktion sichtbar bei konfiguriertem Sync-Server")
                check(tasksPage.syncAction.enabled, "Sync-Aktion aktiv bei konfiguriertem Sync-Server")
                // Aufräumen: ursprüngliche Credentials wiederherstellen.
                app.setSyncCredentials(syncCidZuvor, syncSecretZuvor)
            } else {
                console.log("FLOW-INFO Sync-Positivfall übersprungen (kein Secret Service)")
                app.clearError()
            }
            app.setSyncServerUrlSetting(syncUrlZuvor)

            // 13. Sidebar-Zähler zählen offene Aufgaben, Alle-Zeile liefert
            // offen/gesamt (UI-1, #27). Zwei Aufgaben in Projekt+Tag anlegen,
            // damit Projekt/Tag nach dem Erledigen einer davon sichtbar
            // bleiben (die Sidebar-Liste selbst zeigt nur Projekte/Tags mit
            // mindestens einer aktiven Aufgabe — unverändertes Verhalten).
            check(app.quickCaptureCommit("Flow-Zähltest A +flowcounttest project:flowcounttest"),
                  "Zähltest-Aufgabe A angelegt")
            check(app.quickCaptureCommit("Flow-Zähltest B +flowcounttest project:flowcounttest"),
                  "Zähltest-Aufgabe B angelegt")
            app.applyFilter("project:flowcounttest")
            const zaehlUuids = uuids()
            check(zaehlUuids.length === 2, "beide Zähltest-Aufgaben offen sichtbar")
            const projektVorher = root.projects.find(p => p.name === "flowcounttest").count
            const tagVorher = root.tagList.find(t => t.name === "flowcounttest").count
            const alleOffenVorher = root.counts.all
            const alleGesamtVorher = root.counts.allTotal
            check(projektVorher === 2 && tagVorher === 2,
                  "Projekt-/Tag-Zähler zählen beide offenen Aufgaben")

            app.markDone(zaehlUuids[0])
            check(root.projects.find(p => p.name === "flowcounttest").count === projektVorher - 1,
                  "Projekt-Zähler sinkt nach Erledigen")
            check(root.tagList.find(t => t.name === "flowcounttest").count === tagVorher - 1,
                  "Tag-Zähler sinkt nach Erledigen")
            check(root.counts.allTotal === alleGesamtVorher,
                  "Alle-Gesamt bleibt nach Erledigen gleich")
            check(root.counts.all === alleOffenVorher - 1,
                  "Alle-Offen sinkt nach Erledigen")

            // Aufräumen: Zähltest-Aufgaben entfernen.
            app.deleteTasks(zaehlUuids)

            // 14. Füllfunktion (AI-B1): Entwurf → Dialogfelder als von außen
            // aufrufbare Funktion — Wert→Preset|Custom-Zuordnung in beide
            // Sechser-Fälle (due, recur) plus Datumswähler-Entscheidung.
            // Läuft ohne Mock und ohne KI-Konfiguration.
            quickCaptureDialog.applyDraft({title: "KI-Titel", project: "flowki-projekt",
                                           tags: ["ki", "flow"], due: "tomorrow",
                                           priority: "H", recur: "weekly", notes: "KI-Notiz"})
            let fv = quickCaptureDialog.testValues()
            check(fv.title === "KI-Titel" && fv.project === "flowki-projekt"
                  && fv.tags === "ki flow" && fv.notes === "KI-Notiz",
                  "applyDraft füllt Textfelder")
            check(fv.dueIndex === 2 && fv.priorityIndex === 1 && fv.recurIndex === 2,
                  "applyDraft trifft Presets (tomorrow/H/weekly)")
            quickCaptureDialog.applyDraft({due: "+3d", recur: "quarterly"})
            fv = quickCaptureDialog.testValues()
            check(fv.dueIndex === 5 && fv.dueCustom === "+3d",
                  "freier due-Ausdruck landet im Custom-Feld")
            check(fv.recurIndex === 5 && fv.recurCustom === "quarterly",
                  "freies recur-Intervall landet im Custom-Feld")
            check(fv.title === "KI-Titel", "Entwurf ohne Titel lässt Titel stehen")
            quickCaptureDialog.applyDraft({due: "2027-01-15"})
            fv = quickCaptureDialog.testValues()
            check(fv.dueIndex === 4 && fv.dueDate.getFullYear() === 2027
                  && fv.dueDate.getMonth() === 0 && fv.dueDate.getDate() === 15,
                  "ISO-Datum landet im Datumswähler")
            quickCaptureDialog.applyDraft({})
            fv = quickCaptureDialog.testValues()
            check(fv.project === "" && fv.tags === "" && fv.dueIndex === 0
                  && fv.priorityIndex === 0 && fv.recurIndex === 0 && fv.notes === "",
                  "leerer Entwurf setzt Metadaten-Felder zurück")

            // 15. KI-Gerüst (AI-A3): Property-Defaults der Bridge. Der
            // Worker-Teil (Stale-Drop, Abbruch) läuft nur, wenn der Aufruf
            // eine Mock-Konfiguration mitbringt (Wegwerf-config.json mit
            // ai_model plus VMN_AI_MOCK-Konserve) — sonst wird er
            // übersprungen, damit der Flow nie echte HTTP-Anfragen stellt.
            check(!app.aiBusy, "aiBusy anfangs false")
            check(app.aiError.length === 0, "aiError anfangs leer")
            check(!app.dictationAvailable, "dictationAvailable ohne Sonde false")
            app.cancelAiRequest()
            check(!app.aiBusy && app.aiError.length === 0,
                  "cancelAiRequest ohne laufende Anfrage ist folgenlos")

            if (app.aiConfigured) {
                // Erste Anfrage zieht die künstlich verzögerte Konserve;
                // die zweite folgt zeitversetzt (deterministische Zuordnung).
                app.startAiRequest("erste — wird veraltet")
                check(app.aiBusy, "aiBusy nach startAiRequest true")
                aiSecondRequestTimer.baseFailures = failures
                aiSecondRequestTimer.start()
            } else {
                console.log("FLOW-INFO KI-Worker-Teil übersprungen (aiConfigured false)")
                console.log(failures === 0 ? "FLOW-ENDE: alles grün" : `FLOW-ENDE: ${failures} Fehler`)
                Qt.quit()
            }
        }
    }

    // KI-Flow (AI-A3), Schritt 2: zweite Anfrage nach kurzer Pause — so hat
    // die erste ihre (langsame) Konserve sicher schon gezogen und die
    // Konserven-Zuordnung ist deterministisch.
    Timer {
        id: aiSecondRequestTimer
        property int baseFailures: 0
        interval: 250
        onTriggered: {
            app.startAiRequest("zweite — bleibt aktuell")
            aiResultWaiter.baseFailures = baseFailures
            aiResultWaiter.start()
        }
    }

    // KI-Flow, Schritt 3: Ergebnis der jüngsten Anfrage abwarten, dann den
    // Abbruch-Fall anstoßen.
    Timer {
        id: aiResultWaiter
        property int baseFailures: 0
        property int versuche: 0
        interval: 200
        repeat: true
        onTriggered: {
            versuche++
            if (app.aiBusy && versuche < 25)
                return
            aiResultWaiter.running = false
            let failures = baseFailures
            function check(cond, label) {
                console.log((cond ? "FLOW-OK  " : "FLOW-FAIL ") + label)
                if (!cond) failures++
            }
            check(!app.aiBusy, "KI-Worker meldet fertig (aiBusy false)")
            check(app.aiError.length === 0, "keine aiError-Meldung")
            const antwort = JSON.parse(app.aiResponseJson || "{}")
            check(antwort.marker === "aktuell", "jüngste Antwort publiziert (aiResponseJson)")
            // Abbruch: langsame Konserve starten und sofort abbrechen.
            app.startAiRequest("dritte — wird abgebrochen")
            app.cancelAiRequest()
            check(!app.aiBusy, "cancelAiRequest setzt aiBusy zurück")
            aiCancelWaiter.baseFailures = failures
            aiCancelWaiter.start()
        }
    }

    // KI-Flow, Schritt 4: länger warten, als die langsamen Konserven (800 ms)
    // brauchen — weder das veraltete noch das abgebrochene Ergebnis darf
    // publiziert worden sein (Stale-Drop über den Generationszähler).
    Timer {
        id: aiCancelWaiter
        property int baseFailures: 0
        interval: 1200
        onTriggered: {
            let failures = baseFailures
            function check(cond, label) {
                console.log((cond ? "FLOW-OK  " : "FLOW-FAIL ") + label)
                if (!cond) failures++
            }
            const antwort = JSON.parse(app.aiResponseJson || "{}")
            check(antwort.marker === "aktuell",
                  "veraltete und abgebrochene Ergebnisse verworfen (Stale-Drop)")
            check(app.aiError.length === 0, "kein aiError nach Verwerfen")
            // Weiter zum AI-B1-Teil — der zieht die Konserven 4 und 5.
            aiDraftTimer.baseFailures = failures
            aiDraftTimer.start()
        }
    }

    // KI-Flow, Schritt 5 (AI-B1): „Mit KI interpretieren" end-to-end über den
    // Mock. Hängt bewusst hinter dem KI-Gerüst-Teil: die Konserven-Zuordnung
    // ist positionsabhängig (Gerüst = Konserven 1–3), ein Abschnitt davor
    // würde dessen Indizes verschieben. Erwartete Konserven:
    //   4: {"title": "Zahnarzttermin vereinbaren", "project": "flowki",
    //       "tags": ["gesundheit"], "due": "tomorrow", "priority": "H",
    //       "recur": "monthly", "notes": "Vormittags anrufen"}
    //   5: {"title": "Kaputt-Werte", "project": "", "tags": [],
    //       "due": "übermorgen vielleicht", "priority": "urgent",
    //       "recur": "alle Jubeljahre", "notes": ""}
    Timer {
        id: aiDraftTimer
        property int baseFailures: 0
        interval: 100
        onTriggered: {
            app.startAiInterpret("Zahnarzttermin vereinbaren, nächste Woche, wichtig")
            aiDraftWaiter.baseFailures = baseFailures
            aiDraftWaiter.start()
        }
    }

    // KI-Flow, Schritt 6: validierten Entwurf prüfen, über die Füllfunktion in
    // den Dialog übernehmen und mit dem normalen Anlege-Pfad committen —
    // die KI schlägt nur vor, angelegt wird über addTaskDetailed.
    Timer {
        id: aiDraftWaiter
        property int baseFailures: 0
        property int versuche: 0
        interval: 200
        repeat: true
        onTriggered: {
            versuche++
            if (app.aiBusy && versuche < 25)
                return
            aiDraftWaiter.running = false
            let failures = baseFailures
            function check(cond, label) {
                console.log((cond ? "FLOW-OK  " : "FLOW-FAIL ") + label)
                if (!cond) failures++
            }
            check(!app.aiBusy, "Interpretieren meldet fertig (aiBusy false)")
            check(app.aiError.length === 0, "kein aiError beim Interpretieren")
            const draft = JSON.parse(app.aiDraftJson || "{}")
            check(draft.title === "Zahnarzttermin vereinbaren" && draft.project === "flowki"
                  && draft.due === "tomorrow" && draft.priority === "H"
                  && draft.recur === "monthly",
                  "aiDraftJson trägt validierten Entwurf")
            quickCaptureDialog.applyDraft(draft)
            quickCaptureDialog.commit()
            app.applyFilter("project:flowki")
            const angelegt = Array.from(app.visibleUuids(0, 9999))
            check(angelegt.length === 1, "Entwurf über den normalen Anlege-Pfad committet")
            if (angelegt.length === 1) {
                const t = JSON.parse(app.taskJson(angelegt[0]))
                check(t.project === "flowki" && t.priority === "H" && t.recur === "monthly"
                      && t.due > 0 && t.tags.indexOf("gesundheit") !== -1,
                      "neues Projekt und Entwurfs-Metadaten persistiert")
                check(t.annotations.length === 1
                      && t.annotations[0].description === "Vormittags anrufen",
                      "Entwurfs-Notizen als Annotation")
                app.deleteTasks(angelegt)
            }
            app.applyFilter("all")
            // Konserve 5: unbrauchbare Metadaten müssen leer ankommen.
            app.startAiInterpret("irgendwas mit kaputten Metadaten")
            aiDraftInvalidWaiter.baseFailures = failures
            aiDraftInvalidWaiter.start()
        }
    }

    // KI-Flow, Schritt 7: Validierung — ungültige due/priority/recur-Werte
    // erreichen die Dialogfelder nie (leere Strings statt Müll).
    Timer {
        id: aiDraftInvalidWaiter
        property int baseFailures: 0
        property int versuche: 0
        interval: 200
        repeat: true
        onTriggered: {
            versuche++
            if (app.aiBusy && versuche < 25)
                return
            aiDraftInvalidWaiter.running = false
            let failures = baseFailures
            function check(cond, label) {
                console.log((cond ? "FLOW-OK  " : "FLOW-FAIL ") + label)
                if (!cond) failures++
            }
            const draft = JSON.parse(app.aiDraftJson || "{}")
            check(draft.title === "Kaputt-Werte", "zweiter Entwurf publiziert")
            check(draft.due === "" && draft.priority === "" && draft.recur === "",
                  "ungültige due/priority/recur kommen leer an (Validierung)")
            console.log(failures === 0 ? "FLOW-ENDE: alles grün" : `FLOW-ENDE: ${failures} Fehler`)
            Qt.quit()
        }
    }

    Timer {
        id: testGrabTimer
        property string path: ""
        interval: 3000
        onTriggered: {
            app.grabWindowTo(path)
            Qt.quit()
        }
    }

    Timer {
        id: testDialogTimer
        property string dialogName: ""
        interval: 800
        onTriggered: {
            switch (dialogName) {
            case "quickcapture": quickCaptureDialog.openCapture(); break
            case "settings":
                settingsDialog.openSettings()
                testHoverTimer.start()
                break
            case "help": helpDialog.open(); break
            case "about": aboutDialog.open(); break
            case "detail": {
                const uuids = app.visibleUuids(0, 0)
                if (uuids.length > 0)
                    detailDialog.openFor(uuids[0])
                break
            }
            }
        }
    }

    // Hover vom Suchfeld des Einstellungsfensters wegziehen: das
    // Offscreen-Enter-Event legt die Hover-Position (asynchron, nach dem
    // Öffnen) auf (0,0) über das Suchfeld, dessen Shortcut-Tooltip sonst den
    // --test-grab-Screenshot verschmutzt. Läuft vor dem Grab (3 s).
    // Periodisch, weil die Offscreen-Plattform die Hover-Position immer
    // wieder auf (0,0) zurücksetzt — so läuft die Tooltip-Verzögerung des
    // Suchfelds nie ab.
    Timer {
        id: testHoverTimer
        interval: 300
        repeat: true
        onTriggered: app.testMove(600, 500)
    }

    // Auto-Sync-Intervalle ("immediate" wird Rust-seitig nach Mutationen ausgelöst).
    Timer {
        readonly property var intervals: ({ "m5": 300000, "m15": 900000, "m60": 3600000 })
        interval: intervals[app.autoSyncMode] ?? 0
        running: interval > 0 && app.syncConfigured
        repeat: true
        onTriggered: app.startSync()
    }

    globalDrawer: Sidebar {}

    pageStack.initialPage: TasksPage { id: tasksPage }
    pageStack.defaultColumnWidth: root.width

    DetailDialog {
        id: detailDialog
    }

    QuickCaptureDialog {
        id: quickCaptureDialog
    }

    SettingsDialog {
        id: settingsDialog
        window: root
        appContainer: app
    }

    HelpDialog {
        id: helpDialog
    }

    AboutDialog {
        id: aboutDialog
    }
}
