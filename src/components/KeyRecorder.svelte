<script lang="ts">
  let {
    value = $bindable('Z'),
    disabled = false,
    onChange,
  }: {
    value?: string;
    disabled?: boolean;
    onChange?: (key: string) => void;
  } = $props();

  let isRecording = $state(false);
  let buttonEl = $state<HTMLButtonElement | undefined>(undefined);

  function normalizeKeyCode(e: KeyboardEvent): string | null {
    e.preventDefault();
    e.stopPropagation();

    const code = e.code;

    // Ignore standalone modifier presses while waiting for actual key
    if (
      [
        'ShiftLeft',
        'ShiftRight',
        'ControlLeft',
        'ControlRight',
        'AltLeft',
        'AltRight',
        'MetaLeft',
        'MetaRight',
      ].includes(code)
    ) {
      return null;
    }

    if (code.startsWith('Key')) {
      return code.slice(3).toUpperCase();
    }
    if (code.startsWith('Digit')) {
      return code.slice(5);
    }
    if (code.startsWith('Numpad') && code.length === 7 && !isNaN(Number(code[6]))) {
      return `Numpad${code[6]}`;
    }
    if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(code)) {
      return code.toUpperCase();
    }

    switch (code) {
      case 'Space':
        return 'Space';
      case 'Enter':
      case 'NumpadEnter':
        return 'Enter';
      case 'Escape':
        return 'Escape';
      case 'Tab':
        return 'Tab';
      case 'Backspace':
        return 'Backspace';
      case 'Delete':
        return 'Delete';
      case 'Insert':
        return 'Insert';
      case 'Home':
        return 'Home';
      case 'End':
        return 'End';
      case 'PageUp':
        return 'PageUp';
      case 'PageDown':
        return 'PageDown';
      case 'ArrowUp':
        return 'Up';
      case 'ArrowDown':
        return 'Down';
      case 'ArrowLeft':
        return 'Left';
      case 'ArrowRight':
        return 'Right';
      default:
        if (e.key && e.key.length === 1 && e.key.trim()) {
          return e.key.toUpperCase();
        }
        return null;
    }
  }

  function startRecording() {
    if (disabled) return;
    isRecording = true;
    setTimeout(() => buttonEl?.focus(), 10);
  }

  function stopRecording() {
    isRecording = false;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!isRecording) return;

    const normalized = normalizeKeyCode(e);
    if (normalized) {
      value = normalized;
      isRecording = false;
      if (onChange) onChange(normalized);
    }
  }
</script>

<button
  type="button"
  bind:this={buttonEl}
  class="key-recorder-btn"
  class:recording={isRecording}
  {disabled}
  onclick={isRecording ? stopRecording : startRecording}
  onkeydown={handleKeyDown}
  onblur={stopRecording}
  title={isRecording
    ? 'Нажмите любую клавишу на клавиатуре (букву, цифру, F1-F12, Space...)'
    : 'Нажмите, чтобы назначить клавишу'}
>
  {#if isRecording}
    <span class="recording-pulse" aria-hidden="true">🔴</span>
    <span class="recording-label">Нажмите клавишу...</span>
  {:else}
    <kbd class="keycap">{value || 'Z'}</kbd>
    <span class="key-edit-hint" aria-hidden="true">
      <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor">
        <path
          d="M11.013 1.427a1.75 1.75 0 012.474 0l1.086 1.086a1.75 1.75 0 010 2.474l-8.61 8.61c-.21.21-.47.364-.756.445l-3.251.93a.75.75 0 01-.927-.928l.929-3.25a1.75 1.75 0 01.445-.758l8.61-8.61zm1.414 1.06a.25.25 0 00-.354 0L10.811 3.75l1.439 1.44 1.263-1.263a.25.25 0 000-.354l-1.086-1.086zM11.189 6.25L9.75 4.81 3.06 11.5a.25.25 0 00-.063.108l-.558 1.953 1.953-.558a.25.25 0 00.108-.064l6.689-6.689z"
        />
      </svg>
    </span>
  {/if}
</button>

<style>
  .key-recorder-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
    font-family: inherit;
    color: var(--text);
    transition: all 0.15s ease;
    user-select: none;
    min-height: 34px;
  }

  .key-recorder-btn:hover:not(:disabled) {
    border-color: var(--accent);
    background: rgba(255, 255, 255, 0.08);
  }

  .key-recorder-btn.recording {
    border-color: #ef4444;
    background: rgba(239, 68, 68, 0.12);
    box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.25);
    animation: pulse-border 1.5s infinite;
  }

  .recording-pulse {
    font-size: 10px;
    animation: blink 0.8s infinite;
  }

  .recording-label {
    font-size: 12px;
    font-weight: 600;
    color: #fca5a5;
  }

  .keycap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 24px;
    padding: 2px 7px;
    font-family: 'Fira Code', monospace, sans-serif;
    font-size: 13px;
    font-weight: 700;
    color: #ffffff;
    background: linear-gradient(180deg, #374151 0%, #1f2937 100%);
    border: 1px solid #4b5563;
    border-bottom: 2px solid #111827;
    border-radius: 5px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .key-edit-hint {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
    transition: color 0.15s;
  }

  .key-recorder-btn:hover .key-edit-hint {
    color: var(--accent);
  }

  @keyframes blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }

  @keyframes pulse-border {
    0%,
    100% {
      border-color: #ef4444;
    }
    50% {
      border-color: #f87171;
    }
  }
</style>
