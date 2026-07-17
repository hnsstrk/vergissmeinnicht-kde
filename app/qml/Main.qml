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
                // Hohes Testfenster: alle Formularzeilen sichtbar, damit die
                // synthetischen Klicks ihre Ziele treffen (der Dialog scrollt).
                root.height = 1600
                settingsDialog.openSettings()
                break
            case 2: {
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
                settingsDialog.openSettings()
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
                                     morgen, 0, 0, "M", "weekly"),
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

            // 9. Tag/Projekt-Management
            app.renameTag("flowtest", "flowfertig")
            check(taskOf(folge.uuid).tags.indexOf("flowfertig") !== -1
                  && taskOf(folge.uuid).tags.indexOf("flowtest") === -1, "renameTag")
            app.renameProject("flowdemo", "flowdemo2")
            check(taskOf(folge.uuid).project === "flowdemo2", "renameProject")

            // 10. Aufräumen: alle Flow-Aufgaben löschen
            app.applyFilter("all")
            const opfer = uuids().filter(u => {
                const x = taskOf(u)
                return x.project === "flowdemo2" || (x.tags ?? []).indexOf("flowfertig") !== -1
            })
            app.deleteTasks(opfer)
            app.applyFilter("all")
            check(!uuids().some(u => taskOf(u).project === "flowdemo2"), "Aufräumen")

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
            case "settings": settingsDialog.openSettings(); break
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

    // Auto-Sync-Intervalle ("immediate" wird Rust-seitig nach Mutationen ausgelöst).
    Timer {
        readonly property var intervals: ({ "m5": 300000, "m15": 900000, "m60": 3600000 })
        interval: intervals[app.autoSyncMode] ?? 0
        running: interval > 0 && app.syncConfigured
        repeat: true
        onTriggered: app.startSync()
    }

    globalDrawer: Sidebar {}

    pageStack.initialPage: TasksPage {}
    pageStack.defaultColumnWidth: root.width

    DetailDialog {
        id: detailDialog
    }

    QuickCaptureDialog {
        id: quickCaptureDialog
    }

    SettingsDialog {
        id: settingsDialog
    }

    HelpDialog {
        id: helpDialog
    }

    AboutDialog {
        id: aboutDialog
    }
}
