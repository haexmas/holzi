# Vertrag: Tokens und Kontraste

Welche CSS-Variablen die Darstellung setzt und welche Paare die Prüfung misst. Die Namen sind die
Variablen aus `src/assets/css/tailwind.css` (haex-ui-Tokens); nichts kommt neu hinzu.

## Welcher Regler setzt was

| Regler      | Variablen (beide Schemata, Helligkeit aus dem Standardwert des Schemas)                                                                                                                   |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `accent`    | `--primary`, `--primary-foreground`, `--ring`, `--sidebar-primary`, `--sidebar-primary-foreground`, `--sidebar-ring`                                                                      |
| `window`    | `--background`                                                                                                                                                                            |
| `container` | `--card`, `--popover`, `--sidebar`                                                                                                                                                        |
| `component` | `--secondary`, `--muted`, `--accent`, `--input`, `--border`, `--sidebar-accent`, `--sidebar-border`                                                                                       |
| `text`      | `--foreground`, `--card-foreground`, `--popover-foreground`, `--secondary-foreground`, `--accent-foreground`, `--sidebar-foreground`, `--sidebar-accent-foreground`, `--muted-foreground` |

Nicht angefasst: `--destructive*`, `--error*`, `--success*`, `--warning*`, `--chart-*`, `--radius`.

`--ring` folgt dem Akzent, damit „Ring in Primärfarbe“ (FR-003) auch für Bauteile gilt, die
`ring-ring` benutzen. Die Felder aus haex-ui zeichnen den Fokus als `border-primary` mit einem
halbtransparenten `ring-primary/50`; gemessen wird darum der volle Rand (`--primary`), nicht der
halbtransparente Ring.

## Paare, die `check-appearance.ts` misst

Schwelle „Text“ = 4,5:1, „Bedienelement“ = 3:1. Jedes Paar wird für jedes Farbfeld und jede
Extremwert-Eingabe in `light` und `dark` gemessen.

| Art             | Vordergrund                                             | Hintergrund                                                                              |
| --------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Text            | `--foreground`                                          | `--background`, `--card`, `--popover`, `--sidebar`, `--secondary`, `--muted`, `--accent` |
| Text            | `--card-foreground`                                     | `--card`                                                                                 |
| Text            | `--popover-foreground`                                  | `--popover`                                                                              |
| Text            | `--sidebar-foreground`                                  | `--sidebar`                                                                              |
| Text            | `--secondary-foreground`                                | `--secondary`                                                                            |
| Text            | `--accent-foreground`                                   | `--accent`                                                                               |
| Text (gedämpft) | `--muted-foreground`                                    | `--background`, `--card`, `--muted`                                                      |
| Text auf Akzent | `--primary-foreground`                                  | `--primary`                                                                              |
| Bedienelement   | `--primary` (Schalterspur, Fokusrand, Hinweisumrandung) | `--background`, `--card`, `--popover`, `--sidebar`, `--muted`                            |
| Bedienelement   | `--ring`                                                | `--background`, `--card`, `--popover`, `--sidebar`, `--muted`                            |

Zierränder (`--border`, `--input` gegen ihre Fläche) gehören **nicht** zur Prüfung (Spec FR-016);
ein Eingabefeld erkennt man am Fokusring und am Label, nicht am Rand allein.

Extremwerte einer eigenen Farbe: Sättigung auf der Obergrenze des Reglers (Tönung) beziehungsweise
0.4 (Akzent, danach in den sRGB-Raum begrenzt), Farbton von 0° bis 345° in 15°-Schritten,
dazu die Graustufen (Sättigung 0) und Schwarz/Weiß als Hex-Eingabe.

## Anpassungsreihenfolge (Ableitung)

1. Akzent: Helligkeit lösen (research R4), dann Textfarbe auf Akzent wählen.
2. Tönungen anwenden (Helligkeit = Standard, Sättigung = gedeckelt).
3. Alle Paare messen; für jede Verletzung erst Sättigung der Tönung senken (bis 0), dann
   Helligkeit des Vordergrunds nachführen, bis die Schwelle erreicht ist.
4. Verbleibende Verletzung (darf nicht vorkommen) ist ein Fehler der Prüfung `check-appearance`,
   nicht der App; die App bricht auf die Standardwerte zurück.
