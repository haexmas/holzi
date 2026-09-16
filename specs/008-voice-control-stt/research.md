# Phase 0 Research: Voice Control (Local Speech-to-Text)

Keine `NEEDS CLARIFICATION`-Marker im Technical Context — die Kernentscheidungen wurden bereits im
Brainstorming getroffen (siehe
[docs/plans/2026-09-16-voice-control-stt-design.md](../../docs/plans/2026-09-16-voice-control-stt-design.md)).
Dieses Dokument fasst die Entscheidungen im Research-Format zusammen und ergänzt die zwei Punkte,
die erst während der Planung konkret wurden (Preference-Scope, externes Transkriptions-Protokoll).

## 1. STT-Engine: Whisper über `candle-transformers`

**Decision**: Lokale Transkription läuft über `candle-transformers`s Whisper-Implementierung, auf
demselben `candle-core`/`candle-nn`-Stack (Version 0.10.2), den `mistralrs` für die lokale
Chat-Inferenz bereits in den Dependency-Baum zieht.

**Rationale**: Pure Rust — keine FFI, kein zweiter Cross-Compile-Pfad für iOS/Android neben dem
bestehenden candle-Stack. Teilt CPU/CUDA/Metal-Feature-Flags mit `llm-cpu`/`llm-cuda`/`llm-metal`.
Erfüllt die explizite Anforderung "muss überall laufen, pure Rust".

**Alternatives considered**:

- `whisper-rs` (FFI-Bindings auf whisper.cpp): ausgereifterer/schnellerer C++-Kern, aber eigener
  Build-/Cross-Compile-Pfad für Mobile zusätzlich zum candle-Stack — verworfen, widerspricht der
  "pure Rust überall"-Anforderung.
- `sherpa-onnx`: bietet echtes Streaming (Zipformer), das mit Push-to-Talk (keine Streaming-STT
  nötig) keinen Vorteil bringt; ebenfalls FFI/ONNX-Runtime statt pure Rust.
- Native OS-STT (SFSpeechRecognizer/Android SpeechRecognizer): kein Modell-Bundling nötig, aber
  kein Linux-Äquivalent, inkonsistente Sprachabdeckung, und "on-device" ist nicht über alle
  OS-Versionen hinweg garantiert — Risiko gegen FR-003 (Transkription ohne Netzwerk).

## 2. Audio-Aufnahme: `cpal`

**Decision**: Mikrofon-Aufnahme über `cpal`, plattformübergreifend (Desktop + iOS/Android), mit
einem reinen In-Memory-Puffer. Die Audio-Aufnahme konvertiert an ihrer Grenze jedes native
Geräteformat in den kanonischen Vertrag `CanonicalPcm`: 16.000 Hz, mono, normalisierte `f32`
Samples im Bereich `[-1.0, 1.0]`.

**Rationale**: De-facto-Standard-Audio-I/O-Crate im Rust-Ökosystem, deckt alle Zielplattformen mit
einer API ab. Push-to-Talk (kein Streaming, keine VAD) braucht keine zusätzliche
Audio-Verarbeitung darüber hinaus; der lokale Interrupt-Pfad liest während der Aufnahme dieselben
kanonischen Frames.

`SttAdapter::transcribe` nimmt ausschließlich `&CanonicalPcm` entgegen. Der lokale Whisper-Adapter
verwendet die normalisierten `f32`-Samples direkt. Der externe Adapter kodiert genau dieses Objekt
an der HTTP-Grenze als 16-kHz-mono-signed-16-bit-PCM-WAV im Multipart-Request. Der Tauri-Vertrag
transportiert keine rohen Audiodaten über IPC: Der Puffer bleibt backend-lokal und wird nach dem
Abschluss verworfen.

**Alternatives considered**: Plattformspezifische native APIs direkt anzusprechen — verworfen,
hätte pro Plattform eigenen Code gebraucht, ohne dass Push-to-Talk irgendeinen plattformspezifischen
Vorteil daraus zöge.

## 3. Transkription als Provider-Capability statt Parallelsystem

**Decision**: `Provider`/`ProviderKind` bekommt eine Capability-Unterscheidung (`chat` |
`transcription`); ein schmalerer `SttAdapter`-Trait (`transcribe(audio: &CanonicalPcm) -> Result<String>`) sitzt
neben dem bestehenden `ProviderAdapter` (der auf Chat-Semantik — Streaming, Kontextfenster —
zugeschnitten ist).

**Rationale**: Externe Transkriptions-Dienste sind credential-tragende Konfiguration, konzeptionell
identisch zu dem, was "Modelle verwalten" für Chat-Provider bereits abbildet. Ein Parallelsystem
würde denselben Add/List/Remove-mit-Credentials-Ablauf duplizieren. Per Graphify-Konsultation
(siehe `plan.md` Constitution Check) ist `ProviderAdapter`/`Provider` der bestätigte
Erweiterungskandidat.

**Alternatives considered**: Eigenständiges, paralleles STT-Provider-System (eigene Tabelle, eigene
CRUD-Commands, eigener kleiner Settings-Bereich) — einfacher isoliert zu betrachten, aber
dupliziert einen Nahezu-identischen Ablauf; verworfen nach expliziter Abwägung mit dem Operator.

## 4. Interrupt-Wort-Erkennung: reine Funktion, angebunden an bestehendes Cancellation

**Decision**: Der Interrupt-Matcher ist eine reine, deterministische Funktion (normalisiertes,
vollständiges Transkript gegen `{stop, halt, abbrechen}`). Eine lokale, bounded Fast-Path-Instanz
wendet ihn bereits während der Aufnahme an und löst bei bestätigtem End-of-Utterance direkt das
bestehende `CancellationToken` in `ChatState`/`session.rs` aus — unabhängig vom gewählten
Transkriptions-Provider und vom LLM-/Assistenten-Zustand. Die vollständige STT-Antwort übernimmt
dieses Ergebnis für die Rückgabe an das Frontend, ohne die Cancellation erneut auszulösen.

**Rationale**: Verhindert das Deadlock-Risiko, bei dem ein Stop-Befehl selbst durch den gerade
beschäftigten Assistenten laufen müsste. Kein Adapter, keine Provider-Zugehörigkeit — reine lokale
Textprüfung während bzw. nach der Transkription, vor dem Schreiben ins Eingabefeld. Die normale
Transkription darf daher weiter extern laufen, ohne die Interrupt-Latenz zu bestimmen.

**Alternatives considered**: LLM-basiertes Intent-Parsing für alle Sprachbefehle inkl. Stop —
verworfen wegen des Deadlock-Risikos während laufender Generierung (siehe Design-Dokument §"one
technical risk").

## 5. Geräteklassen-Tier für das gebündelte Modell

**Decision**: Die Wahl der Whisper-Modellgröße (tiny/base/small) nutzt dieselbe
Hardware-Erkennung, die spec 002 bereits für die Qwen3-Tier-Auswahl (4B Desktop / 1.7B Mobile /
0.6B Low-Memory) einführt — keine neue Hardware-Erkennung.

**Rationale**: Direkte Wiederverwendung einer bereits gelösten, getesteten Klassifikation.
Vermeidet zwei parallele "welche Hardware ist das"-Implementierungen.

## 6. Preference-Scope: Auto-Send vault-weit, aktive Transkriptionsquelle geräte-lokal

**Decision**: Der Auto-Send-Toggle wird als vault-weite Preference gespeichert (`PrefScope::Vault`,
Namespace `voice.auto_send`); die aktive Transkriptionsquelle (lokal vs. externer Provider) wird
pro Gerät gespeichert (`PrefScope::Device`, Namespace `voice.active_stt_provider`).

**Rationale**: Auto-Send ist eine reine UX-Präferenz, sinnvollerweise überall gleich. Die aktive
Transkriptionsquelle ist dagegen an Geräteeigenschaften gekoppelt (Akku/Netzwerk auf Mobile vs.
Desktop) — ein Nutzer kann auf dem Smartphone bewusst bei der gebündelten lokalen Option bleiben
und auf dem Desktop einen externen Dienst aktivieren. Nutzt die bestehende
`storage::preferences`-Infrastruktur (`PrefScope::Vault`/`PrefScope::Device`) ohne neues Schema.

**Alternatives considered**: Beides vault-weit — einfacher, aber zwingt dieselbe
Transkriptionsquelle auf alle Geräte unabhängig von deren Hardware/Netzwerksituation; verworfen.

## 7. Externes Transkriptions-Protokoll

**Decision**: `ExternalSttAdapter` spricht ein einzelnes, multipart-basiertes
HTTP-Transkriptions-Protokoll (Audio-Datei + Modellname im Request-Body, Text im Response-Body) —
kompatibel mit dem inzwischen quasi-standardisierten `/v1/audio/transcriptions`-Formular, das
mehrere Anbieter (u. a. OpenAI-kompatible Endpunkte) unterstützen. `base_url` bleibt
konfigurierbar, sodass auch Selbst-Host-/kompatible Endpunkte funktionieren, ohne dass ein
zweites Protokoll gebaut werden muss.

**Rationale**: Ein Protokoll statt eines Katalogs pro Anbieter hält den ersten Schnitt klein und
deckt die gängigsten Fälle ab. Deckt sich mit FR-012 ("einen externen Transkriptions-Dienst
konfigurieren"), ohne eine Mehrfach-Vendor-Abstraktion vorwegzunehmen, die niemand angefordert
hat.

**Alternatives considered**: Vendor-spezifischer Adapter-Katalog analog zu den Chat-`api_key`-
Vendoren (Anthropic, künftig OpenAI/Google/Groq) — bewusst zurückgestellt; kann später ergänzt
werden, sobald ein zweiter, inkompatibler Anbieter konkret gebraucht wird.
