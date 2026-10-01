# Contract: failure material for several devices

Extends the failure material of spec 016 (see `specs/016-e2e-testing/contracts/report.md`).

```text
<runDir>/<scenario>/
├── timeline.json            # steps; each step names the device it touched
├── nostr-relay.log          # as today
├── <device>/                # one folder per device of the group, e.g. anna-laptop/
│   ├── driver.log           # that device's driver and application output
│   ├── screenshot.png       # taken at failure while the device lives; screenshot-note.txt otherwise
│   └── data/                # the device's data folder, kept on failure or with --keep
```

- Folder names replace `/` in `<user>/<device>` by `-`.
- A passing scenario's folder is removed unless `--keep`, as today.
- `report.json` lists, per failed scenario, the devices and the files kept.
- CI uploads `src-tauri/target/e2e` as today; the layout above is inside it.
