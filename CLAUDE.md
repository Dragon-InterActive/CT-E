# Projekt Name und Beschreibung

`CT-Editor` ist der Name für den Cargo TOML Editor des Projekts Freya-TD. Seine Funktion ist die Erstellung und Modifizierung der carg.toml mittels Visuellem Editor.
Hierbei soll der Editor auf die cargo-eigenen Funktionen zurückgreifen um das Hinzufügen und Editieren von Crates zu ermöglichen und dabei die Syntax der toml beachtet und überwacht. Neue Crates sollen über ein Such-Feld gesucht und ausgewählt werden können. Gewählte Crates werden dann mit Kurzbeschreibung und ihren Features aufgelistet. Features können dann per checkbox ausgewählt werden. Default ist "alle".

Optisch soll der Editor 2 Tabs haben. Tab#1 ist der Quell-Code der cargo.toml und Tab#2 ist die visuelle Schicht.

## Umsetzung und Nutzung von Crates

Wichtig ist, dass der Editor i18n (Multi-Language) ist. Die aktuelle Sprache und der Pfad wird beim Aufruf übergeben, da der Editor später als Teil des gesamten Freya-TD sein wird und entsprechend von der Oberfläche aus aufgerufen wird.

RUF(Rust native UI Framework) wurde in Rust erstellt und arbeitet ohne Qt6 Libraray. Im Unterverzeichnis ../RUF findes du alles weitere und unter ../RUF/.claude/skills findest du auch alle benötigten Skills um RUF für den Editor zu benutzen.
Weitere Crates, welche dir als sinnvoll erscheinen, sollen installiert und benutzt werden. Die Entscheidung liegt vollständig bei dir.

### Version und Github

Mein Github-Name ist Dragon-InterActive und der Git-Upload erfolgt über das gesamt-Projekt Freya-TD.
Rust-Version ist 1.99.1 also min.-Version auf 1.99

### Dokumentation und AI

Erstelle eine Dokumentation in deutsch und englisch mit Beispielen.
Bei der Entwicklung berücksichtige die Nutzung von KI.

## Lizenz

Das gesamte Projekt steht unter der Doppellizenz MIT OR Apache-2.0 (nach Wahl des Nutzers).
