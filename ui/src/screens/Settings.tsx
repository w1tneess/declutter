import { useEffect, useState } from 'react';
import { api, errorText } from '../api/client';
import type { BinInfo } from '../api/generated/BinInfo';
import type { Preset } from '../api/generated/Preset';
import type { ReminderStatus } from '../api/generated/ReminderStatus';
import type { SpaceFreed } from '../api/generated/SpaceFreed';
import { Badge, Button, Card, Icon, NumberField, Segmented, Switch, TextField } from '../design/components';
import { humanSize, plural } from '../format';
import { confirm, toast, updateSettings, useApp } from '../state/store';
import { ScreenHeader } from './common';

function Row({ label, desc, children }: { label: string; desc?: string; children: React.ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span className="setting-label">{label}</span>
        {desc && <span className="setting-desc">{desc}</span>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

function PresetEditor({ preset, onDone }: { preset: Preset; onDone: () => void }) {
  const [draft, setDraft] = useState(preset);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState<Record<number, string>>({});
  const setRule = (i: number, change: Partial<Preset['rules'][number]>) =>
    setDraft((d) => ({ ...d, rules: d.rules.map((r, k) => (k === i ? { ...r, ...change } : r)) }));
  const addExt = (i: number) => {
    const raw = (adding[i] ?? '').trim().toLowerCase();
    if (!raw) return;
    const ext = raw.startsWith('.') ? raw : `.${raw}`;
    const rule = draft.rules[i];
    if (rule && !rule.extensions.includes(ext)) setRule(i, { extensions: [...rule.extensions, ext] });
    setAdding((a) => ({ ...a, [i]: '' }));
  };
  const save = async () => {
    try {
      useApp.setState({ presets: await api.savePreset(draft) });
      toast(`Saved the preset “${draft.name}”.`);
      onDone();
    } catch (e) {
      setError(errorText(e));
    }
  };
  return (
    <div className="preset-editor">
      <TextField label="Preset name" value={draft.name} onChange={(name) => setDraft({ ...draft, name })} />
      {draft.rules.map((r, i) => (
        <fieldset key={i} className="rule">
          <legend>Rule {i + 1}</legend>
          <TextField label="Folder" hint="Inside the chosen folder, up to two levels, like Work\Invoices" value={r.folder} onChange={(folder) => setRule(i, { folder })} />
          <div className="field">
            <span className="field-label" id={`ext-${i}`}>File types</span>
            <ul className="chips" aria-labelledby={`ext-${i}`}>
              {r.extensions.map((e) => (
                <li key={e} className="chip">
                  {e}
                  <button type="button" aria-label={`Remove ${e}`} onClick={() => setRule(i, { extensions: r.extensions.filter((x) => x !== e) })}><Icon name="dismiss" size={16} /></button>
                </li>
              ))}
            </ul>
            <div className="chip-add">
              <input className="input" aria-label={`Add a file type to rule ${i + 1}`} placeholder=".pdf" value={adding[i] ?? ''} onChange={(e) => setAdding({ ...adding, [i]: e.target.value })} onKeyDown={(e) => e.key === 'Enter' && addExt(i)} />
              <Button icon="add" onClick={() => addExt(i)}>Add</Button>
            </div>
          </div>
          <Button variant="subtle" onClick={() => setDraft({ ...draft, rules: draft.rules.filter((_, k) => k !== i) })}>Remove rule</Button>
        </fieldset>
      ))}
      <Button icon="add" onClick={() => setDraft({ ...draft, rules: [...draft.rules, { extensions: [], folder: '' }] })}>Add rule</Button>
      {error && <p className="field-error" role="alert"><Icon name="errorCircle" size={16} />{error}</p>}
      <div className="row-actions">
        <Button variant="primary" onClick={() => void save()}>Save preset</Button>
        <Button onClick={onDone}>Cancel</Button>
      </div>
    </div>
  );
}

export function Settings() {
  const settings = useApp((s) => s.settings);
  const presets = useApp((s) => s.presets);
  const info = useApp((s) => s.info);
  const [freed, setFreed] = useState<SpaceFreed | null>(null);
  const [bin, setBin] = useState<BinInfo | null>(null);
  const [reminder, setReminder] = useState<ReminderStatus | null>(null);
  const [editing, setEditing] = useState<Preset | null>(null);

  useEffect(() => {
    api.spaceFreed().then(setFreed, () => undefined);
    api.binInfo().then(setBin, () => undefined);
    api.reminderStatus().then(setReminder, () => undefined);
  }, []);

  if (!settings) return null;
  const run = async <T,>(p: Promise<T>, then: (v: T) => void) => {
    try {
      then(await p);
    } catch (e) {
      toast(errorText(e));
    }
  };
  const addPin = async () => {
    const p = await api.pickFolder().catch(() => null);
    if (p) await run(api.addPin(p), (s) => useApp.setState({ settings: s }));
  };
  const removePreset = async (p: Preset) => {
    if (await confirm({ title: 'Delete preset?', body: `Delete the preset “${p.name}”? Your files are not affected.`, confirmLabel: 'Delete preset', variant: 'danger' }))
      await run(api.deletePreset(p.id), (presets) => {
        useApp.setState({ presets });
        if (settings.activePreset === p.id) void updateSettings({ activePreset: null });
      });
  };

  return (
    <div className="screen screen-scroll">
      <ScreenHeader title="Settings" subtitle="Every change is saved at once." />
      <div className="settings">
        <Card>
          <h2 className="card-title">Appearance</h2>
          <Row label="Theme" desc="System follows your Windows setting.">
            <Segmented label="Theme" value={settings.theme as 'system' | 'light' | 'dark'} onChange={(theme) => void updateSettings({ theme })} options={[{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }]} />
          </Row>
        </Card>

        <Card>
          <h2 className="card-title">Start-up</h2>
          <Switch label="Reopen the last folder" description="Picks it again at start. It is not scanned until you press Scan." checked={settings.rememberLastFolder} onChange={(on) => void updateSettings({ rememberLastFolder: on, lastFolder: on ? useApp.getState().folder : null })} />
        </Card>

        <Card>
          <h2 className="card-title">Scanning</h2>
          <div className="field-row">
            <NumberField label="Leave files touched in the last (hours)" hint="0 to 72, 0 turns it off" value={settings.recentHours} min={0} max={72} onCommit={(n) => void updateSettings({ recentHours: n })} />
            <NumberField label="Large files: bigger than (MB)" hint="0 turns this filter off" value={settings.minMb} min={0} onCommit={(n) => void updateSettings({ minMb: n })} />
            <NumberField label="Old files: older than (days)" hint="0 turns this filter off" value={settings.olderDays} min={0} onCommit={(n) => void updateSettings({ olderDays: n })} />
          </div>
        </Card>

        <Card>
          <h2 className="card-title">Never touch</h2>
          <Row label="Pinned folders" desc="Skipped by every scan. Never moved or sent to the Recycle Bin.">
            <Button icon="add" onClick={() => void addPin()}>Add folder…</Button>
          </Row>
          {settings.pins.length ? (
            <ul className="plain-list pins">
              {settings.pins.map((p) => (
                <li key={p}>
                  <Icon name="pin" size={16} />
                  <span className="path">{p}</span>
                  <Button variant="subtle" aria-label={`Unpin ${p}`} onClick={() => void run(api.removePin(p), (s) => useApp.setState({ settings: s }))}>Remove</Button>
                </li>
              ))}
            </ul>
          ) : <p className="hint">Nothing pinned yet.</p>}
        </Card>

        <Card>
          <h2 className="card-title">Sort presets</h2>
          <Row label="Your own sort rules" desc="Choose which file types go to which folder. Pick a preset on the Sort tab.">
            <Button icon="add" onClick={() => setEditing({ id: '', name: '', builtin: false, rules: [{ extensions: [], folder: '' }] })}>New preset</Button>
          </Row>
          {editing && <PresetEditor key={editing.id || 'new'} preset={editing} onDone={() => setEditing(null)} />}
          <ul className="plain-list presets">
            {presets.map((p) => (
              <li key={p.id}>
                <span className="preset-name">{p.name}</span>
                {p.builtin && <Badge>Built-in</Badge>}
                <span className="setting-desc">{p.rules.map((r) => `${r.folder}: ${r.extensions.join(' ')}`).join(' · ')}</span>
                {p.builtin ? (
                  <Button variant="subtle" onClick={() => setEditing({ ...p, id: '', builtin: false, name: `${p.name} (copy)` })}>Make a copy</Button>
                ) : (
                  <>
                    <Button variant="subtle" onClick={() => setEditing(p)}>Edit</Button>
                    <Button variant="subtle" onClick={() => void removePreset(p)}>Delete</Button>
                  </>
                )}
              </li>
            ))}
          </ul>
        </Card>

        <Card>
          <h2 className="card-title">Reminder</h2>
          <Switch
            label="Weekly reminder"
            description={`Once a week Declutter looks through your folder and shows a notification if there is something to tidy. It only scans: it never moves, deletes or changes a file.${reminder?.enabled && reminder.nextRun ? ` Next check: ${reminder.nextRun}.` : ''}`}
            checked={reminder?.enabled ?? false}
            onChange={(on) => void run(on ? api.reminderEnable('weekly') : api.reminderDisable(), (r) => {
              setReminder(r);
              void updateSettings({ reminderEnabled: r.enabled });
            })}
          />
        </Card>

        <Card>
          <h2 className="card-title">Space and Recycle Bin</h2>
          <Row label="Space freed with Declutter" desc={freed ? `${humanSize(freed.bytes)} in ${plural(freed.files, 'file')}${freed.since ? ` since ${freed.since}` : ''}. Counted on this computer only.` : 'Loading…'}>
            <Button onClick={() => void confirm({ title: 'Reset the tally?', body: 'Start counting freed space from zero? Your files are not affected.', confirmLabel: 'Reset' }).then((ok) => { if (ok) void run(api.resetSpaceFreed(), setFreed); })}>Reset</Button>
          </Row>
          <Row label="Recycle Bin" desc={bin ? `Holds ${humanSize(bin.bytes)} in ${plural(bin.items, 'item')}. Restore or empty it in File Explorer.` : 'Loading…'}>
            <Button icon="delete" onClick={() => void run(api.openRecycleBin(), () => undefined)}>Open Recycle Bin</Button>
          </Row>
        </Card>

        <Card>
          <h2 className="card-title">About</h2>
          <Row label={`Declutter ${info?.version ?? ''}`} desc="Free and open source (MIT). Your files never leave this computer.">
            <span />
          </Row>
        </Card>
      </div>
    </div>
  );
}
