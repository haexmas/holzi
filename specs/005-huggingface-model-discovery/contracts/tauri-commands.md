# Tauri-Vertrag: Hugging-Face-Discovery und Installation

Alle Rückgaben verwenden `camelCase`. Das Backend liefert strukturierte Werte
und Fehlercodes bzw. Parameter, keine lokalisierten UI-Texte.

## `search_huggingface_models`

**Args**:

```typescript
{
  query: string,
  page?: number,
  limit?: number
}
```

**Returns**: `HuggingFaceModelResult[]`.

Die Query wird getrimmt und auf mindestens zwei Zeichen validiert. Das Backend
führt nur öffentliche Repository-Suche aus, begrenzt die Seite auf höchstens 20
Treffer und sortiert Treffer deterministisch. Treffer ohne installierbare
GGUF-Datei werden nicht zurückgegeben; falls Datei-Details einen zweiten
Request benötigen, liefert der Command zunächst normalisierte Repository-
Treffer mit `files` nach der Detailanreicherung. Exakte Katalogtreffer tragen
`catalogMatch: true` und `catalogEntryId`, bleiben aber eigenständige
Suchresultate.

**Fehler**: strukturierte `InvalidInput`, `Network`, `Timeout`, `HttpStatus`
oder `RateLimited`-Fehler. Ein Fehler darf die vorherige Frontend-Ergebnisliste
nicht löschen.

## `get_huggingface_model_details`

**Args**:

```typescript
{
  repoId: string,
  revision?: string
}
```

**Returns**: ein `HuggingFaceModelResult` mit allen installierbaren
`HuggingFaceFileCandidate`s, Dateigrößen soweit verfügbar, normalisierter
Quantisierung, Kontextfenster, Metadaten-Provenienz und Tokenizer-Hinweisen.

Repository-ID und Revision werden vor dem HTTP-Aufruf validiert. Private oder
gated Antworten werden nicht als installierbares Ergebnis ausgegeben.

## `preview_huggingface_install`

**Args**:

```typescript
{
  repoId: string,
  filename: string,
  revision?: string,
  tokenizerRepo?: string,
  contextWindow?: number | null
}
```

**Returns**:

```typescript
{
  modelId: string,
  name: string,
  repoId: string,
  filename: string,
  revision: string,
  revisionRef: string | null,
  sizeBytes: number | null,
  quantization: string | null,
  contextWindow: number | null,
  tokenizerRepo: string | null,
  tokenizerRequired: boolean,
  catalogMatch: boolean,
  catalogEntryId: string | null,
  metadataProvenance: {
    quantization: 'hub_metadata' | 'filename_heuristic' | 'gguf_header' | 'unknown',
    contextWindow: 'hub_metadata' | 'filename_heuristic' | 'gguf_header' | 'unknown'
  },
  fit: 'fits' | 'tight' | 'too_big' | 'unknown',
  requiresExplicitTooBigConfirmation: boolean
}
```

Der Preview-Command lädt keine Modelldatei herunter und schreibt keine
Persistenz. Er validiert Quelle/Dateiname, löst eine mutable Revision in eine
konkrete Commit-SHA auf, berechnet die lokale ID und ermittelt die
Hardware-Warnung. Ein fehlendes Tokenizer-Repository wird als
`tokenizerRequired: true` ausgegeben. Eine nicht auflösbare Revision führt zu
einem strukturierten Fehler.

## `download_model_from_hf` (bestehend, erweitert)

Der bestehende Command bleibt der einzige Downloadpfad für Hugging-Face-GGUFs.
Die Argumente werden um Source-/Revision-Felder erweitert oder aus dem
Preview-Contract intern erzeugt. Die Commit-SHA ist nach der Auflösung
verpflichtend:

```typescript
{
  id: string,
  name: string,
  hfRepo: string,
  hfFilename: string,
  hfRevision: string,
  hfRevisionRef?: string | null,
  tokenizerRepo: string,
  contextWindow?: number | null,
  forceTooBig?: boolean
}
```

Verhalten:

1. Repository-ID, Dateiname, Commit-SHA, optionaler Upstream-Ref, Slug und
   Tokenizer werden an der Trust-Boundary validiert. Ein Download ohne
   aufgelöste Commit-SHA ist ungültig.
2. `TooBig` wird ohne `forceTooBig === true` abgelehnt; `Fits`, `Tight` und
   `Unknown` behalten die im Preview ausgewiesene Semantik.
3. Der Download schreibt in eine temporäre Datei, emittiert den bestehenden
   `model-download-progress`-Payload und veröffentlicht erst nach vollständigem
   Erfolg atomar.
4. Danach wird genau eine gemeinsame `models`-Zeile mit Source-Metadaten
   sowie dem berechneten `fileSha256` registriert und `model-download-complete`
   emittiert.
5. Fehler hinterlassen keinen installierten Eintrag. Bereits installierte
   Modelle und deren Metadaten bleiben erhalten.
6. Eine identische bereits installierte Quelle wird idempotent behandelt. Ein
   vorhandener Slug mit anderer finaler GGUF-Datei wird als Konflikt abgelehnt.
7. Ein Update mit derselben Modell-ID ersetzt die bestehende Datei erst nach
   erfolgreicher atomarer Veröffentlichung und schreibt danach die neue SHA;
   bei Fehlern bleiben alte Datei und Metadaten gültig.

**Fehler**: `InvalidInput`, `UnsupportedFormat`, `TokenizerRequired`,
`HardwareConfirmationRequired`, `Network`, `Timeout`, `HttpStatus`, `Io` oder
`ModelRegistrationFailed` mit maschinenlesbaren Parametern.

## Bestehende Commands ohne Contract-Bruch

- `list_installed_models`: bleibt autoritativ für lokale Dateien und liefert
  auch frei installierte Modelle. Jeder Eintrag liefert zusätzlich
  `fileSha256` und `integrityStatus`.
- `load_model`: lädt über dieselbe lokale ID und denselben kanonischen Pfad;
  keine erneute Hub-Suche. Vor dem Load gilt die Integritätsprüfung oben.
- `delete_installed_model`: löscht nur die lokale Datei; Source-/Provider-
  Metadaten bleiben gemäß Spec 002 erhalten.
- `download_model_from_catalog`: bleibt auf kuratierte Qwen3-Einträge begrenzt
  und verwendet weiterhin den gemeinsamen HF-Downloadpfad.
- `check_huggingface_model_updates`: prüft installierte HF-Modelle mit
  gespeichertem `hfRevisionRef` gegen Hugging Faces aktuellen Commit und
  liefert `HuggingFaceUpdateStatus[]`. Offline-/HTTP-Fehler sind retrybar und
  ändern keine lokalen Modelldaten.

`HuggingFaceUpdateStatus`:

```typescript
{
  modelId: string,
  repoId: string,
  revisionRef: string,
  installedRevision: string,
  latestRevision: string | null,
  updateAvailable: boolean,
  checkedAt: string,
  errorCode: string | null
}
```

## `load_model` und lokale Integritätsprüfung

`load_model` behält die bestehende Modell-ID als Argument. Für lokale Modelle
führt das Backend unmittelbar vor dem Runtime-Load diese Reihenfolge aus:

1. kanonische lokale Datei auflösen;
2. vollständigen SHA-256-Hash der Datei in einem Blocking-Task berechnen;
3. mit `models.file_sha256` vergleichen;
4. nur bei exakter Übereinstimmung den normalen Runtime-Loader starten.

Bei fehlender Datei, fehlendem Hash, Hash-Mismatch oder Hashing-Fehler startet
kein normaler Load. Stattdessen wird ein strukturierter Fehler geliefert:

```typescript
{
  code: 'ModelIntegrityMismatch' | 'ModelIntegrityUnknown' | 'ModelIntegrityError',
  modelId: string,
  expectedSha256: string | null,
  actualSha256: string | null,
  options: ['load_untrusted', 'repair_source', 'choose_other']
}
```

Die UI muss diesen Fehler als Entscheidungsdialog darstellen. Die explizite
Aktion `load_untrusted` ruft einen separaten, bestätigungspflichtigen Pfad
`load_model_with_integrity_override(modelId)` auf. Dieser markiert
`integrityStatus = 'untrusted'`, behält `fileSha256` unverändert und lädt die
aktuell vorgefundene Datei. `repair_source` verwendet bei HF-Modellen den
gespeicherten Source-Contract und öffnet bei importierten Modellen den lokalen
Dateiimport; `choose_other` öffnet den Modell-Picker. Keine dieser Aktionen darf
die erwartete SHA stillschweigend aktualisieren.
- `model-download-progress` und `model-download-complete`: Payload-Struktur
  bleibt kompatibel.

## Frontend-Composable-Vertrag

`useHuggingFace()` kapselt:

```typescript
searchAsync(query: string, page?: number): Promise<HuggingFaceModelResult[]>
detailsAsync(repoId: string, revision?: string): Promise<HuggingFaceModelResult>
previewInstallAsync(args: PreviewInstallArgs): Promise<InstallPreview>
downloadAsync(args: DownloadFromHfArgs): Promise<InstalledModel>
checkUpdatesAsync(): Promise<HuggingFaceUpdateStatus[]>
```

Alle Promises werden von aufrufenden Komponenten behandelt; bei unmount werden
Event-Listener wie beim bestehenden `useModels`-Composable entfernt.
