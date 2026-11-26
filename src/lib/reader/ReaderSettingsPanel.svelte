<script lang="ts">
  // Reader settings, LNReader-style Reader / General tabs (same options as the Android sheet;
  // no TTS by request). A side drawer on desktop instead of a bottom sheet.
  import Icon from "$lib/components/Icon.svelte";
  import { READER_LIMITS, READER_THEMES, settings, type ReaderFont, type ReaderTheme, type TextAlign } from "$lib/settings.svelte";

  let { onClose }: { onClose: () => void } = $props();
  let tab = $state<"reader" | "general">("reader");
  const v = $derived(settings.values);

  const themes: { id: ReaderTheme; label: string; bg: string; fg: string }[] = [
    { id: "follow", label: "System", bg: "var(--background)", fg: "var(--on-background)" },
    ...Object.entries(READER_THEMES).map(([id, c]) => ({ id: id as ReaderTheme, label: id[0].toUpperCase() + id.slice(1), ...c })),
  ];
  const aligns: { id: TextAlign; icon: string }[] = [
    { id: "left", icon: "format_align_left" },
    { id: "center", icon: "format_align_center" },
    { id: "justify", icon: "format_align_justify" },
    { id: "right", icon: "format_align_right" },
  ];
  const fonts: { id: ReaderFont; label: string; css: string }[] = [
    { id: "original", label: "Original", css: "var(--font-ui)" },
    { id: "serif", label: "Serif", css: "Georgia, serif" },
    { id: "sans", label: "Sans Serif", css: "Arial, sans-serif" },
    { id: "mono", label: "Monospace", css: "Consolas, monospace" },
    { id: "lora", label: "Lora", css: "Lora, serif" },
    { id: "nunito", label: "Nunito", css: "Nunito, sans-serif" },
  ];

  type NumKey = "fontSize" | "lineSpacing" | "contentPadding" | "autoScrollSpeed" | "columnWidth";
  function step(key: NumKey, delta: number) {
    const [min, max] = READER_LIMITS[key];
    const next = Math.round((v[key] + delta) * 10) / 10;
    settings.set(key, Math.min(max, Math.max(min, next)));
  }

  type BoolKey = "fullscreen" | "showBatteryAndTime" | "showVerticalSeekbar" | "removeExtraSpacing" | "bionicReading" | "autoScroll";
  const toggles: { key: BoolKey; label: string; sub?: string }[] = [
    { key: "fullscreen", label: "Fullscreen", sub: "F11 also toggles it" },
    { key: "showBatteryAndTime", label: "Battery & time", sub: "Battery, progress and clock strip at the bottom" },
    { key: "showVerticalSeekbar", label: "Vertical seekbar" },
    { key: "removeExtraSpacing", label: "Remove extra spacing" },
    { key: "bionicReading", label: "Bionic reading" },
    { key: "autoScroll", label: "Auto-scroll", sub: "Space pauses and resumes" },
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={onClose}></div>
<aside class="panel">
  <header>
    <div class="tabs">
      <button class:on={tab === "reader"} onclick={() => (tab = "reader")}>Reader</button>
      <button class:on={tab === "general"} onclick={() => (tab = "general")}>General</button>
    </div>
    <button class="icon-btn" title="Close" onclick={onClose}><Icon name="close" /></button>
  </header>

  <div class="body">
    {#if tab === "reader"}
      <div class="row">
        <span>Text size</span>
        <div class="stepper">
          <button class="icon-btn" onclick={() => step("fontSize", -1)}>−</button>
          <span class="val">{v.fontSize}</span>
          <button class="icon-btn" onclick={() => step("fontSize", 1)}>+</button>
        </div>
      </div>

      <div class="block">
        <span>Color</span>
        <div class="swatches">
          {#each themes as t}
            <button class="swatch" class:on={v.readerTheme === t.id} style:background={t.bg} style:color={t.fg} title={t.label} onclick={() => settings.set("readerTheme", t.id)}>
              {#if v.readerTheme === t.id}<Icon name="check" size={20} />{:else}Aa{/if}
            </button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span>Text align</span>
        <div class="seg">
          {#each aligns as a}
            <button class:on={v.textAlign === a.id} title={a.id} onclick={() => settings.set("textAlign", a.id)}><Icon name={a.icon} size={20} /></button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span>Line height</span>
        <div class="stepper">
          <button class="icon-btn" onclick={() => step("lineSpacing", -0.1)}>−</button>
          <span class="val">{v.lineSpacing.toFixed(1)}</span>
          <button class="icon-btn" onclick={() => step("lineSpacing", 0.1)}>+</button>
        </div>
      </div>

      <div class="row">
        <span>Padding</span>
        <div class="stepper">
          <button class="icon-btn" onclick={() => step("contentPadding", -2)}>−</button>
          <span class="val">{v.contentPadding}</span>
          <button class="icon-btn" onclick={() => step("contentPadding", 2)}>+</button>
        </div>
      </div>

      <div class="row">
        <span>Text width <span class="body-small muted">(desktop)</span></span>
        <div class="stepper">
          <button class="icon-btn" onclick={() => step("columnWidth", -40)}>−</button>
          <span class="val">{v.columnWidth}</span>
          <button class="icon-btn" onclick={() => step("columnWidth", 40)}>+</button>
        </div>
      </div>

      <div class="block">
        <span>Font style</span>
        <div class="fonts">
          {#each fonts as f}
            <button class="chip" class:selected={v.fontFamily === f.id} style:font-family={f.css} onclick={() => settings.set("fontFamily", f.id)}>{f.label}</button>
          {/each}
        </div>
      </div>
    {:else}
      {#each toggles as t}
        <label class="toggle">
          <span class="lbl">
            <span>{t.label}</span>
            {#if t.sub}<span class="body-small muted">{t.sub}</span>{/if}
          </span>
          <input type="checkbox" checked={v[t.key]} onchange={(e) => settings.set(t.key, (e.target as HTMLInputElement).checked)} />
          <span class="switch"></span>
        </label>
      {/each}
      {#if v.autoScroll}
        <div class="row">
          <span>Auto-scroll speed</span>
          <div class="stepper">
            <button class="icon-btn" onclick={() => step("autoScrollSpeed", -1)}>−</button>
            <span class="val">{v.autoScrollSpeed}</span>
            <button class="icon-btn" onclick={() => step("autoScrollSpeed", 1)}>+</button>
          </div>
        </div>
      {/if}
      <div class="keys body-small muted">
        <div class="label-medium">Keyboard</div>
        ↑ ↓ / wheel scroll · PgUp PgDn / Space page · ← → chapter · Home End · C chapters · S settings · F11 fullscreen · Esc back
      </div>
    {/if}
  </div>
</aside>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.3);
  }
  .panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 41;
    width: min(400px, 90vw);
    display: flex;
    flex-direction: column;
    background: var(--surface-container-low);
    color: var(--on-surface);
    box-shadow: -4px 0 16px rgb(0 0 0 / 0.3);
    animation: slide 220ms var(--ease);
  }
  header {
    display: flex;
    align-items: center;
    padding: 8px;
    border-bottom: 1px solid var(--outline-variant);
  }
  .tabs { display: flex; flex: 1; }
  .tabs button {
    flex: 1;
    height: 44px;
    border: none;
    background: none;
    font-weight: 500;
    color: var(--on-surface-variant);
    border-bottom: 3px solid transparent;
  }
  .tabs button.on { color: var(--primary); border-bottom-color: var(--primary); }
  .body { flex: 1; overflow-y: auto; padding: 8px 20px 20px; }
  .row, .toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 56px;
  }
  .block { display: flex; flex-direction: column; gap: 10px; padding: 10px 0; }
  .stepper { display: flex; align-items: center; gap: 4px; }
  .stepper .icon-btn { font-size: 20px; color: var(--on-surface); }
  .val { min-width: 40px; text-align: center; font-variant-numeric: tabular-nums; }
  .swatches { display: flex; flex-wrap: wrap; gap: 10px; }
  .swatch {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    border: 1px solid var(--outline-variant);
    font-weight: 600;
  }
  .swatch.on { box-shadow: 0 0 0 3px var(--primary); }
  .seg { display: flex; border: 1px solid var(--outline); border-radius: 20px; overflow: hidden; }
  .seg button {
    display: grid;
    place-items: center;
    width: 48px;
    height: 36px;
    border: none;
    background: none;
    color: var(--on-surface-variant);
  }
  .seg button + button { border-left: 1px solid var(--outline); }
  .seg button.on { background: var(--secondary-container); color: var(--on-secondary-container); }
  .fonts { display: flex; flex-wrap: wrap; gap: 8px; }
  .toggle { cursor: pointer; }
  .lbl { display: flex; flex-direction: column; }
  .toggle input { position: absolute; opacity: 0; pointer-events: none; }
  .switch {
    position: relative;
    width: 52px;
    height: 32px;
    flex: none;
    border-radius: 16px;
    background: var(--surface-container-highest);
    border: 2px solid var(--outline);
    transition: background 150ms;
  }
  .switch::after {
    content: "";
    position: absolute;
    top: 6px;
    left: 6px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--outline);
    transition: all 150ms var(--ease);
  }
  .toggle input:checked + .switch { background: var(--primary); border-color: var(--primary); }
  .toggle input:checked + .switch::after { left: 22px; top: 2px; width: 24px; height: 24px; background: var(--on-primary); }
  .keys { margin-top: 16px; line-height: 1.8; }
  @keyframes slide { from { transform: translateX(100%); } }
</style>
