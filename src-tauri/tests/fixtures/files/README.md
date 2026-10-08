# File browser fixtures (spec 044, T040)

Media for `scripts/e2e/scenarios/files-media.test.ts` (read through `scripts/e2e/lib/file-fixtures.ts`). Generated once from synthetic sources (test
pattern, sine tone, hand-written PDF), so they carry no third-party rights; CI does not need ffmpeg.

| File        | Content                                     | Made with                                                                                                                                                                                                                                 |
| ----------- | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `film.mp4`  | 12 s, 320×240, H.264 baseline + AAC mono    | `ffmpeg -f lavfi -i testsrc=size=320x240:rate=15:duration=12 -f lavfi -i sine=frequency=440:duration=12 -c:v libx264 -profile:v baseline -preset veryslow -crf 36 -pix_fmt yuv420p -c:a aac -b:a 32k -ac 1 -movflags +faststart film.mp4` |
| `ton.mp3`   | 10 s, 330 Hz sine, 64 kbit/s mono           | `ffmpeg -f lavfi -i sine=frequency=330:duration=10 -c:a libmp3lame -b:a 64k -ac 1 ton.mp3`                                                                                                                                                |
| `brief.pdf` | two pages, "Seite 1" / "Seite 2", Helvetica | a short script writing the objects and the xref table by hand                                                                                                                                                                             |
