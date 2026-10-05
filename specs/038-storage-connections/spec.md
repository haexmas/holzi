# Feature Specification: Speicherverbindungen (S3) und ihre Weitergabe an Erweiterungen

**Feature Branch**: `038-storage-connections`

**Created**: 2026-10-05

**Status**: Draft

**Input**: User description: "Speicherverbindungen (S3) in holzi und ihre Weitergabe an
Erweiterungen. Grundsatz des Betreibers: holzi spricht die Protokolle selbst (IMAP, SMTP, S3, ICS …)
und reicht sie über Berechtigungen an Erweiterungen weiter; Erweiterungen bekommen nie Zugangsdaten.
Die Speicherverbindung aus Spec 029 wird herausgezogen, ohne Spaces; 029 nutzt sie später. Umfang:
Verbindungen anlegen, ändern, prüfen, entfernen (Zugangsdaten im Passwortmanager 034); ein S3-Client in
holzi; Erweiterungen erreichen eine Verbindung nur mit Berechtigung je Speicher und unter einem eigenen
Schlüsselpräfix (Spec 017 T106, die neun Funktionen `extension_remote_storage_*` des vault-sdk)."

## Begriffe

- **Speicherverbindung**: Anbieter, Endpunkt, Region, Adressierung (wie Buckets in der Adresse
  stehen) und die **Zugangsdaten** bei diesem Anbieter. Derselbe Begriff wie in Spec 029; diese Spec
  legt ihn fest, 029 baut darauf auf.
- **Speicher**: ein Bucket auf einer Speicherverbindung, mit einem Namen, den der Nutzer vergibt.
  Ihn sieht eine Erweiterung als „Backend“ des vault-sdk. Eine Speicherverbindung kann mehrere
  Speicher tragen.
- **Verbindungstest**: die Prüfung, ob holzi mit den Zugangsdaten den Endpunkt erreicht und im Bucket
  eines Speichers auflisten, schreiben, lesen und löschen kann. Er ist kleiner als die
  Eignungsprüfung von 029 (keine Versionierung, keine eingeschränkten Zugangsschlüssel).
- **Bereich einer Erweiterung**: der Teil eines Speichers, den eine Erweiterung sieht: alle Objekte
  unter einem Schlüsselpräfix, das nur ihr gehört. Die Erweiterung sieht die Schlüssel ohne dieses
  Präfix.
- **Erweiterung**, **Berechtigung** (Zustand erteilt, verweigert, fragen), **Dialog von holzi**: wie in
  Spec 017.
- **Passwortmanager**: Spec 034. Er verwahrt die Zugangsdaten der Speicherverbindungen.

## Beziehung zu bestehenden Specs

- **Grundsatz**: holzi spricht die Protokolle selbst (IMAP, SMTP, S3, ICS …) und gibt sie über
  Berechtigungen an Erweiterungen weiter. Eine Erweiterung bekommt nie Zugangsdaten, nur das, was ihre
  Berechtigung erlaubt. Diese Spec setzt das für S3 um.
- **Spec 017** (Erweiterungs-Host): Diese Spec erfüllt T106, User Story 10 (Szenarien 2 bis 4) und
  FR-054 und FR-055 dort. Die Berechtigungsart `remoteStorage` mit Ziel „ein Speicher“ und den
  Aktionen Lesen sowie Lesen und Schreiben gibt es dort schon. Bis diese Spec gebaut ist, antworten
  die neun Funktionen „nicht verfügbar“ (8001).
- **Spec 029** (eigener S3-Speicher für Spaces): 029 nutzt die Speicherverbindung dieser Spec,
  ergänzt um ihre Eignungsprüfung, Space-Buckets und Zugangsschlüssel. Was 029 über die Verbindung
  festlegt (gehört der Vault, liegt im Passwortmanager, synchronisiert auf alle eigenen Geräte,
  FR-033 dort), gilt hier genauso.
- **Spec 034** (Passwortmanager): verwahrt die Zugangsdaten. Wie Erweiterungen den Passwortmanager
  sonst nutzen, bleibt bei 034 und Spec 017 (US10).
- **Spec 024** (Sync der eigenen Geräte): Speicherverbindungen und Speicher sind gewöhnliche
  Vault-Daten und kommen mit dem Datensync auf alle eigenen Geräte.

## Clarifications

### Session 2026-10-05

- Q: Gilt eine Speicherverbindung vault-weit oder nur auf einem Gerät? → A: Vault-weit, wie in 029
  (FR-033 dort): Sie liegt im Passwortmanager und kommt mit dem Datensync auf alle eigenen Geräte.
- Q: Darf eine Erweiterung einen Speicher anlegen, ändern, prüfen oder entfernen? → A: Ja, aber jedes
  Mal nur nach Bestätigung in einem Dialog von holzi (Spec 017 FR-055).
- Q: Wer gibt die Zugangsdaten ein, wenn eine Erweiterung einen Speicher vorschlägt? → A: Nur der
  Nutzer, in holzi. Zugangsdaten werden ausschließlich in den Einstellungen oder im Dialog von holzi
  angelegt und geändert und landen direkt im Passwortmanager. Eine Erweiterung kann einen neuen
  Anbieter oder Bucket anstoßen und dafür Name, Anbieter, Endpunkt, Region und Bucket vorschlagen,
  aber keine Zugangsdaten; ein Aufruf mit Zugangsdaten wird abgelehnt.
- Q: Was sieht eine Erweiterung von den Speichern? → A: Mit einer Leseberechtigung für S3-Speicher nur
  die Namen: den Namen des Speichers, den Namen des Anbieters und den Namen des Buckets, damit sie sie
  als Auswahl anbieten kann. Endpunkt, Region und Zugangsdaten sieht sie nicht.

## User Scenarios & Testing _(mandatory)_

### User Story 1 - Eigenen Speicher in holzi verbinden (Priority: P1)

Anna hat einen Bucket bei einem S3-kompatiblen Anbieter. In den Einstellungen von holzi legt sie eine
Speicherverbindung an (Endpunkt, Region, Zugangsdaten) und darauf einen Speicher für ihren Bucket.
holzi testet die Verbindung, bevor es etwas speichert, und zeigt das Ergebnis. Sie kann den Speicher
später umbenennen, die Zugangsdaten erneuern, erneut testen und ihn entfernen.

**Why this priority**: Ohne Speicher gibt es nichts, das holzi an Erweiterungen weitergeben könnte,
und 029 braucht dieselbe Verbindung.

**Independent Test**: Gegen einen lokalen S3-kompatiblen Testserver eine Verbindung und einen Speicher
anlegen, testen, umbenennen, die Zugangsdaten tauschen, entfernen. Danach liegen keine Zugangsdaten
mehr in der Vault.

**Acceptance Scenarios**:

1. **Given** gültige Zugangsdaten und ein erreichbarer Endpunkt, **When** Anna eine Verbindung mit
   einem Speicher anlegt, **Then** testet holzi den Speicher, speichert Verbindung und Speicher erst
   danach, und die Zugangsdaten liegen im Passwortmanager.
2. **Given** falsche Zugangsdaten, ein nicht erreichbarer Endpunkt oder ein fehlender Bucket, **When**
   Anna speichern will, **Then** nennt holzi den Grund verständlich und speichert nichts.
3. **Given** eine gespeicherte Verbindung, **When** Anna einen zweiten Speicher (anderer Bucket) darauf
   anlegt, **Then** muss sie die Zugangsdaten nicht erneut eingeben.
4. **Given** ein gespeicherter Speicher, **When** Anna ihn öffnet, **Then** sieht sie Name, Anbieter,
   Endpunkt, Region und Bucket, aber nie das Geheimnis der Zugangsdaten im Klartext ohne eigene
   Aktion (wie bei Passwörtern in 034).
5. **Given** eine Verbindung mit zwei Speichern, **When** Anna die Verbindung entfernen will, **Then**
   nennt holzi die Speicher und die Erweiterungen mit Berechtigung darauf, und nach Bestätigung sind
   Verbindung, Speicher, Zugangsdaten und diese Berechtigungen weg. Die Objekte beim Anbieter bleiben.

---

### User Story 2 - Eine Erweiterung nutzt einen freigegebenen Speicher (Priority: P1)

Eine Backup-Erweiterung lädt Dateien in Annas Speicher hoch, listet sie auf, lädt sie herunter und
löscht alte. Sie sieht nur ihren eigenen Bereich in diesem Speicher und keinen anderen Speicher, und
sie erfährt nie die Zugangsdaten.

**Why this priority**: Das ist der Zweck von T106: das Protokoll in holzi, die Nutzung in der
Erweiterung.

**Independent Test**: Eine Test-Erweiterung mit Berechtigung „Lesen und Schreiben“ für einen Speicher
lädt hoch, listet, lädt herunter und löscht. Ein Objekt, das ein anderer in denselben Bucket gelegt
hat, taucht in ihrer Liste nicht auf. Ein zweiter Speicher ohne Berechtigung ist für sie nicht
erreichbar.

**Acceptance Scenarios**:

1. **Given** eine erteilte Berechtigung „Lesen und Schreiben“ für einen Speicher, **When** die
   Erweiterung hochlädt, auflistet, herunterlädt und löscht, **Then** gelingt alles, und die
   Schlüssel, die sie sieht, sind die, die sie selbst geschrieben hat.
2. **Given** nur „Lesen“, **When** die Erweiterung hochladen oder löschen will, **Then** wird das
   abgelehnt; Auflisten und Herunterladen gelingen.
3. **Given** zwei Erweiterungen mit Berechtigung für denselben Speicher, **When** beide auflisten,
   **Then** sieht jede nur ihren eigenen Bereich, und keine kann ein Objekt der anderen lesen, ändern
   oder löschen, auch nicht über einen Schlüssel mit `..` oder einem führenden `/`.
4. **Given** eine Erweiterung mit Leseberechtigung, **When** sie die Speicher auflistet, **Then**
   sieht sie genau die Speicher, die ihre Leseberechtigungen decken (alle bei einer Berechtigung für
   `*`), jeweils nur mit Kennung, Name des Speichers, Name des Anbieters und Name des Buckets, nie mit
   Endpunkt, Region oder Zugangsdaten, und kann sie als Auswahl anbieten.
5. **Given** eine Erweiterung ohne Leseberechtigung für S3-Speicher, **When** sie die Speicher
   auflistet, **Then** erhält sie eine leere Liste oder die Rückfrage nach Spec 017, nie Namen.
6. **Given** ein Speicher im Zustand „fragen“, **When** die Erweiterung ihn nutzt, **Then** fragt holzi
   wie bei jeder Berechtigung (Spec 017), und eine Verweigerung beendet den Aufruf.
7. **Given** ein Objekt, das größer ist als die Grenzwerte der Erweiterung erlauben, **When** sie es
   hoch- oder herunterladen will, **Then** wird der Aufruf mit einem eigenen Fehler abgelehnt, und
   holzi bleibt stabil.

---

### User Story 3 - Eine Erweiterung stößt einen neuen Speicher an (Priority: P2)

Eine Erweiterung bietet an, einen neuen Anbieter oder Bucket einzurichten. Sie ruft dazu die Funktion
des SDK auf und schlägt Name, Anbieter, Endpunkt, Region und Bucket vor, oder einen neuen Bucket auf
einem schon verbundenen Anbieter. holzi öffnet seinen eigenen Dialog; dort prüft Anna die Angaben und
gibt die Zugangsdaten ein, nur hier. holzi testet, legt die Zugangsdaten direkt im Passwortmanager ab
und gibt der Erweiterung danach eine Berechtigung für genau diesen Speicher. Ändern, Testen und
Entfernen durch eine Erweiterung laufen ebenso über einen Dialog von holzi.

**Why this priority**: Komfort für Erweiterungen, die einen eigenen Speicher brauchen; Anna kann
denselben Speicher auch in den Einstellungen anlegen (US1) und freigeben.

**Independent Test**: Die Test-Erweiterung ruft „Speicher hinzufügen“ ohne Zugangsdaten auf; im Dialog
die Zugangsdaten eingeben und bestätigen (Speicher angelegt, Zugangsdaten im Passwortmanager,
Berechtigung erteilt) oder abbrechen (nichts angelegt). Derselbe Aufruf mit Zugangsdaten wird
abgelehnt, ohne dass ein Dialog erscheint.

**Acceptance Scenarios**:

1. **Given** eine Erweiterung ruft „Speicher hinzufügen“ mit Name, Anbieter, Endpunkt, Region und
   Bucket auf, **When** Anna im Dialog von holzi die Zugangsdaten eingibt und bestätigt, **Then** testet
   holzi den Speicher (wie US1), legt die Zugangsdaten im Passwortmanager ab, legt Verbindung und
   Speicher an, erteilt der Erweiterung „Lesen und Schreiben“ dafür und gibt ihr die Kennung des
   Speichers zurück.
2. **Given** eine Erweiterung schlägt einen neuen Bucket auf einem schon verbundenen Anbieter vor,
   **When** Anna bestätigt, **Then** nutzt holzi die vorhandenen Zugangsdaten, ohne dass Anna sie neu
   eingibt.
3. **Given** derselbe Aufruf, **When** Anna abbricht oder der Test scheitert, **Then** wird nichts
   angelegt, und die Erweiterung erhält einen Fehler ohne Zugangsdaten.
4. **Given** eine Erweiterung schickt Zugangsdaten mit (beim Hinzufügen oder Ändern), **When** holzi
   den Aufruf erhält, **Then** lehnt holzi ihn ab, bevor ein Dialog erscheint, und speichert nichts.
5. **Given** ein Speicher, **When** eine Erweiterung ihn ändern (Name, Bucket), testen oder entfernen
   will, **Then** geht das nur mit einer Berechtigung „Lesen und Schreiben“ für diesen Speicher und
   nur nach Bestätigung im Dialog von holzi; neue Zugangsdaten gibt Anna dort ein.
6. **Given** eine Erweiterung entfernt einen Speicher, den auch andere Erweiterungen nutzen, **When**
   der Dialog erscheint, **Then** nennt er diese Erweiterungen.
7. **Given** die Zugangsdaten eines Speichers, **When** eine Erweiterung irgendeine Funktion nutzt,
   auch die Passwort-Funktionen mit einer Freigabe für `*`, **Then** erreicht sie die Zugangsdaten nie.

---

### User Story 4 - Speicher auf allen eigenen Geräten (Priority: P2)

Anna richtet den Speicher auf dem Laptop ein. Auf dem Telefon steht er nach dem Sync der Vault bereit,
und eine Erweiterung mit Berechtigung kann ihn dort ebenso nutzen.

**Why this priority**: Folgt aus der vault-weiten Verbindung; ohne sie müsste Anna jeden Speicher je
Gerät neu einrichten.

**Independent Test**: Zwei Geräte derselben Vault; auf Gerät 1 einen Speicher anlegen, nach dem Sync
auf Gerät 2 testen und von einer Erweiterung dort hochladen lassen.

**Acceptance Scenarios**:

1. **Given** ein Speicher auf Gerät 1, **When** der Datensync gelaufen ist, **Then** steht er auf
   Gerät 2 mit Zugangsdaten bereit, ohne erneute Eingabe.
2. **Given** ein auf Gerät 1 entfernter Speicher, **When** der Sync gelaufen ist, **Then** ist er auf
   Gerät 2 weg, mit seinen Zugangsdaten und den Berechtigungen dafür.
3. **Given** holzi auf Android oder iOS, **When** eine Erweiterung einen Speicher nutzt, **Then** geht
   das wie am Desktop.

### Edge Cases

- **Zugangsdaten beim Anbieter widerrufen oder abgelaufen**: Aufrufe der Erweiterung scheitern mit
  einem Fehler „Zugang abgelehnt“, ohne Details der Zugangsdaten; in den Einstellungen zeigt holzi
  am Speicher, dass neue Zugangsdaten nötig sind.
- **Anbieter nicht erreichbar**: Aufruf scheitert mit einem Netzwerkfehler; holzi bleibt stabil, kein
  Aufruf hängt länger als die Laufzeitgrenze der Erweiterung.
- **Bucket wird beim Anbieter gelöscht**: Aufrufe scheitern mit „nicht gefunden“; der Speicher bleibt,
  bis Anna ihn entfernt.
- **Schlüssel einer Erweiterung, der aus ihrem Bereich führt** (`..`, führendes `/`, leere Teile,
  Steuerzeichen, zu lang): wird abgelehnt, bevor holzi den Anbieter fragt.
- **Sehr viele Objekte im Bereich**: Auflisten liefert seitenweise oder bis zu einer Grenze und sagt,
  dass es mehr gibt.
- **Dieselbe Verbindung, zweites Gerät ändert gleichzeitig**: Es gilt die zuletzt synchronisierte
  Änderung; ein Gerät mit veralteten Zugangsdaten zeigt den Fehler aus dem ersten Punkt.
- **Erweiterung wird entfernt**: Ihre Berechtigungen gehen wie in Spec 017; ihre Objekte beim
  Anbieter bleiben.
- **Objekt mit dem Namen eines Präfixes anderer Erweiterungen**: kann es nicht geben, die Bereiche
  überschneiden sich nie.

## Requirements _(mandatory)_

### Functional Requirements

**Speicherverbindungen und Speicher**

- **FR-001**: Der Nutzer MUSS in den Einstellungen von holzi Speicherverbindungen zu S3-kompatiblen
  Anbietern anlegen, ändern und entfernen können: Endpunkt, Region, Adressierung und Zugangsdaten
  (Zugangsschlüssel und Geheimnis, optional ein Sitzungstoken).
- **FR-002**: Der Nutzer MUSS auf einer Verbindung Speicher anlegen, umbenennen und entfernen können:
  je Speicher ein Name und ein Bucket. Mehrere Speicher DÜRFEN sich eine Verbindung teilen; die
  Zugangsdaten werden dann nicht erneut eingegeben.
- **FR-003**: Vor dem ersten Speichern und nach jeder Änderung der Zugangsdaten, des Endpunkts oder
  des Buckets MUSS holzi den Verbindungstest ausführen. Schlägt er fehl, MUSS holzi den Grund
  verständlich nennen (Zugangsdaten falsch, Endpunkt nicht erreichbar, Bucket fehlt, Recht fehlt)
  und DARF nichts speichern. Ein Test DARF beim Anbieter nichts außer seinem eigenen Testobjekt
  zurücklassen.
- **FR-004**: Der Nutzer MUSS einen gespeicherten Speicher jederzeit erneut testen können.
- **FR-005**: Die Zugangsdaten MÜSSEN im Passwortmanager (034) liegen und DÜRFEN sonst nirgends in
  lesbarer Form gespeichert werden. Die Einstellungen zeigen das Geheimnis nur auf eine eigene Aktion
  des Nutzers, wie Passwörter in 034.
- **FR-006**: Speicherverbindungen und Speicher MÜSSEN Vault-Daten sein und mit dem Datensync (Spec 024)
  auf alle eigenen Geräte gelangen.
- **FR-007**: Beim Entfernen einer Verbindung oder eines Speichers MUSS holzi vorher die betroffenen
  Speicher und die Erweiterungen mit Berechtigung darauf nennen. Danach MÜSSEN die Zugangsdaten (wenn
  keine Verbindung sie mehr nutzt) und alle Berechtigungen für die betroffenen Speicher entfernt sein.
  Objekte und Buckets beim Anbieter DARF holzi dabei NICHT löschen.

**Weitergabe an Erweiterungen**

- **FR-008**: holzi MUSS die neun Funktionen `extension_remote_storage_*` des vault-sdk anbieten:
  Speicher auflisten, hinzufügen, ändern, testen, entfernen sowie Objekte hochladen, herunterladen,
  auflisten und löschen. Ein Speicher der SDK-Funktionen ist ein Speicher dieser Spec.
- **FR-009**: Eine Erweiterung MUSS für einen Speicher eine Berechtigung der Art `remoteStorage`
  brauchen (Spec 017), mit dem Speicher oder `*` als Ziel. „Lesen“ erlaubt, den Speicher in der Liste
  zu sehen, Objekte aufzulisten und herunterzuladen, „Lesen und Schreiben“ zusätzlich Hochladen und
  Löschen. Ein Speicher, den keine Berechtigung der Erweiterung deckt, DARF für sie weder in der Liste
  erscheinen noch erreichbar sein. Zustände und Rückfragen folgen Spec 017.
- **FR-009a**: Die Liste der Speicher für eine Erweiterung MUSS je Speicher nur Kennung, Namen des
  Speichers, Namen des Anbieters und Namen des Buckets enthalten, nie Endpunkt, Region oder
  Zugangsdaten.
- **FR-010**: Jede Erweiterung MUSS in jedem Speicher einen eigenen Bereich haben: alle ihre
  Schlüssel liegen unter einem Präfix, das nur ihr gehört, auf allen Geräten gleich ist und eine
  Neuinstallation derselben Erweiterung überdauert. Die Erweiterung sieht und nennt Schlüssel ohne
  dieses Präfix. Kein Aufruf DARF ein Objekt außerhalb ihres Bereichs auflisten, lesen, schreiben
  oder löschen.
- **FR-011**: holzi MUSS jeden Schlüssel einer Erweiterung vor dem Aufruf beim Anbieter prüfen und
  Schlüssel ablehnen, die aus dem Bereich führen oder ungültig sind (`..`, führendes `/`, leere Teile,
  Steuerzeichen, mehr als die Längengrenze des Anbieters zusammen mit dem Präfix).
- **FR-012**: Eine Erweiterung DARF Zugangsdaten nie erreichen: weder in Antworten der Speicher-
  Funktionen noch in Fehlermeldungen noch über die Passwort-Funktionen (Spec 017 US10), auch nicht
  mit einer Freigabe für alle Einträge.
- **FR-013**: Hinzufügen, Ändern, Testen und Entfernen eines Speichers durch eine Erweiterung MUSS jedes
  Mal eine Bestätigung in einem Dialog von holzi verlangen (Spec 017 FR-055). Ändern, Testen und
  Entfernen MÜSSEN zusätzlich eine Berechtigung „Lesen und Schreiben“ für diesen Speicher verlangen.
  Ein bestätigtes Hinzufügen MUSS der Erweiterung „Lesen und Schreiben“ für den neuen Speicher
  erteilen.
- **FR-013a**: Zugangsdaten DÜRFEN nur in holzi angelegt und geändert werden: in den Einstellungen
  (FR-001) oder im Dialog von holzi, den eine Erweiterung anstößt. holzi MUSS sie direkt im
  Passwortmanager ablegen. Eine Erweiterung DARF beim Hinzufügen nur Name, Anbieter, Endpunkt, Region
  und Bucket vorschlagen, oder einen neuen Bucket auf einer vorhandenen Verbindung; ein Aufruf, der
  Zugangsdaten enthält, MUSS abgelehnt werden, bevor ein Dialog erscheint.
- **FR-014**: Hoch- und Herunterladen MÜSSEN die Größen- und Zeitgrenzen der Erweiterung (Spec 017)
  einhalten; ein Verstoß ist ein eigener Fehler.
- **FR-015**: Fehler beim Anbieter MÜSSEN als wenige verständliche Fehlerarten bei der Erweiterung
  ankommen (nicht gefunden, Zugang abgelehnt, Netzwerkfehler, Grenze überschritten), ohne Endpunkt-
  Details, die Zugangsdaten enthalten könnten.
- **FR-016**: Die Funktionen MÜSSEN auf allen Plattformen von holzi gleich arbeiten, auch auf Android
  und iOS.

**Protokoll in holzi**

- **FR-017**: holzi MUSS das S3-Protokoll selbst sprechen; eine Erweiterung erreicht den Anbieter nur
  über holzi. Verschlüsselte Verbindungen zum Anbieter MÜSSEN die Zertifikate prüfen; unverschlüsselte
  Endpunkte (`http`) DÜRFEN nur für Adressen im lokalen Netz oder auf dem eigenen Rechner erlaubt sein
  und MÜSSEN im Dialog als unverschlüsselt gekennzeichnet sein.

### Key Entities

- **Speicherverbindung**: Anbieter, Endpunkt, Region, Adressierung, Verweis auf die Zugangsdaten im
  Passwortmanager. Gehört der Vault, synchronisiert auf alle eigenen Geräte. Von 029 mitgenutzt.
- **Speicher**: Name, Bucket, zugehörige Speicherverbindung, Zeitpunkt der Anlage, Ergebnis des
  letzten Tests. Ziel der Berechtigungen der Art `remoteStorage`.
- **Bereich einer Erweiterung**: abgeleitet aus Speicher und Identität der Erweiterung; wird nicht
  gespeichert.
- **Berechtigung `remoteStorage`**: wie in Spec 017, mit einem Speicher als Ziel.

## Success Criteria _(mandatory)_

### Measurable Outcomes

- **SC-001**: Mit bereitliegenden Zugangsdaten richtet ein Nutzer einen Speicher in unter 2 Minuten ein
  und sieht das Testergebnis in unter 10 Sekunden bei erreichbarem Anbieter.
- **SC-002**: In 100 % der geprüften Fälle (Antworten, Fehlermeldungen, Passwort-Funktionen mit
  Freigabe für alle Einträge, Liste der Speicher) erreicht eine Erweiterung keine Zugangsdaten.
- **SC-003**: In 100 % der Fälle eines Prüfkatalogs aus Schlüsseln, die aus dem Bereich führen sollen,
  erreicht eine Erweiterung kein Objekt außerhalb ihres Bereichs.
- **SC-004**: Ein gescheiterter Verbindungstest hinterlässt in 100 % der Fälle keine Zugangsdaten in der
  Vault und kein Testobjekt beim Anbieter.
- **SC-005**: Ein auf einem Gerät angelegter Speicher ist nach dem Sync auf einem zweiten Gerät ohne
  erneute Eingabe nutzbar.
- **SC-006**: Die neun Funktionen arbeiten nachweislich mit mindestens zwei Anbietern: RustFS (selbst
  betrieben, auch im Test) und AWS S3.

## Assumptions

- Ein **Speicher** ist ein vorhandener Bucket; holzi legt in dieser Spec keine Buckets an und löscht
  keine. Das Anlegen von Buckets gehört zu 029.
- Der **Verbindungstest** schreibt, liest, listet und löscht ein eigenes kleines Testobjekt im Bucket;
  Versionierung und eingeschränkte Zugangsschlüssel prüft erst die Eignungsprüfung von 029.
- Das **Präfix** einer Erweiterung leitet sich aus ihrer dauerhaften Identität ab (wie ihre Tabellen
  in Spec 017), nicht aus einer Kennung je Installation.
- Die **Grenzen** für Größe und Laufzeit sind die vorhandenen Grenzwerte der Erweiterung aus Spec 017;
  diese Spec führt keine neuen ein.
- Das **vault-sdk** schickt beim Hinzufügen heute Zugangsdaten mit (`S3Config`); es wird angepasst,
  sodass eine Erweiterung nur noch Vorschläge ohne Zugangsdaten schickt (eigener PR im vault-sdk).
- Die **Zugangsdaten** sind Zugangsschlüssel und Geheimnis, optional ein Sitzungstoken. Anmeldeverfahren
  der Anbieter jenseits davon (etwa Anmelden im Browser) gehören nicht dazu.
- **haex-files** ist obsolet und kein Maßstab für Verhalten oder Oberfläche.

## Nicht im Umfang

- Spaces, Space-Buckets, Zugangsschlüssel je Space, Eignungsprüfung und Versionierung (Spec 029).
- Ein Dateibrowser oder Dateisync von holzi auf einem Speicher (Specs 025 und 029).
- Andere Protokolle (IMAP, SMTP, ICS, WebDAV, FTP). Sie folgen demselben Grundsatz in eigenen Specs.
- Verschlüsselung der Objekte durch holzi; was eine Erweiterung hochlädt, liegt beim Anbieter so, wie
  sie es schickt.
