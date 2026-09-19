import { Button } from "../ui/button";
import { Slider } from "../ui/slider";
import { cn } from "../../lib/cn";
import { invoke } from "../../lib/gameOverlayBridge";
import { ImageIcon, Trash2, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { normalizeKeyInput } from "../../overlay/input";
import { settingValue } from "../../overlay/moduleRuntime";

export function KeyCaptureButton({ active, value, onStart, onCancel, onCapture }) {
  useEffect(() => {
    if (!active) return undefined;

    const handleKeyDown = (event) => {
      event.preventDefault();
      event.stopPropagation();
      const nextKey = normalizeKeyInput(event);
      if (!nextKey) return;
      onCapture(nextKey);
    };
    const handlePointerDown = (event) => {
      if (event.target?.closest?.("[data-key-capture-button='true']")) return;
      onCancel();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("pointerdown", handlePointerDown, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("pointerdown", handlePointerDown, true);
    };
  }, [active, onCancel, onCapture]);

  const displayValue = String(value || "");
  return (
    <button
      type="button"
      data-key-capture-button="true"
      onClick={onStart}
      className={cn(
        "flex h-9 w-full items-center justify-between rounded border bg-black/25 px-3 text-left text-sm text-white outline-none transition",
        active ? "border-[var(--theme-accent)] shadow-[0_0_0_2px_rgba(255,255,255,0.08)]" : "border-white/10",
      )}
    >
      <span className={displayValue || active ? "text-white" : "text-white/35"}>
        {active ? "Press shortcut..." : displayValue || "Click to record"}
      </span>
      <span className="text-[11px] text-white/35">{active ? "Recording" : "Key"}</span>
    </button>
  );
}

export function ResetButton({ onClick }) {
  return (
    <button
      type="button"
      className="rounded border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-white/55 hover:bg-white/10 hover:text-white"
      onClick={onClick}
    >
      Reset
    </button>
  );
}

export function ImageSettingInput({ value, onChange, onReset }) {
  const fileInputRef = useRef(null);
  const fileDialogActiveRef = useRef(false);
  const hasImage = typeof value === "string" && value.startsWith("data:image/");

  useEffect(() => {
    return () => {
      if (fileDialogActiveRef.current) {
        invoke("set_game_overlay_file_dialog_active", { active: false }).catch(console.error);
      }
    };
  }, []);

  function setFileDialogActive(active) {
    fileDialogActiveRef.current = active;
    invoke("set_game_overlay_file_dialog_active", { active }).catch(console.error);
  }

  function pickImage() {
    setFileDialogActive(true);
    const clearAfterFocus = () => {
      window.setTimeout(() => {
        if (fileDialogActiveRef.current) {
          setFileDialogActive(false);
        }
      }, 250);
    };
    window.addEventListener("focus", clearAfterFocus, { once: true });
    fileInputRef.current?.click();
  }

  function handleFileChange(event) {
    setFileDialogActive(false);
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || !file.type.startsWith("image/")) return;

    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") {
        onChange(reader.result);
      }
    };
    reader.readAsDataURL(file);
  }

  return (
    <div className="rounded border border-white/10 bg-black/20 p-3">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        onChange={handleFileChange}
        className="hidden"
      />
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-sm text-white/80">
          <ImageIcon className="h-4 w-4 text-white/45" />
          <span>{hasImage ? "Image selected" : "No image selected"}</span>
        </div>
        <ResetButton onClick={onReset} />
      </div>
      {hasImage ? (
        <div className="mb-3 overflow-hidden rounded border border-white/10 bg-black/35">
          <img src={value} alt="" className="max-h-40 w-full object-contain" />
        </div>
      ) : null}
      <div className="flex gap-2">
        <Button type="button" variant="secondary" size="sm" className="h-9 flex-1 rounded" onClick={pickImage}>
          <Upload className="h-4 w-4" />
          Upload
        </Button>
        {hasImage ? (
          <Button type="button" variant="secondary" size="sm" className="h-9 rounded px-3" onClick={() => onChange("")}>
            <Trash2 className="h-4 w-4" />
          </Button>
        ) : null}
      </div>
    </div>
  );
}

export function ImagesSettingInput({ value, onChange, onReset }) {
  const fileInputRef = useRef(null);
  const fileDialogActiveRef = useRef(false);
  const images = Array.isArray(value) ? value.filter((item) => typeof item === "string" && item.startsWith("data:image/")) : [];

  useEffect(() => {
    return () => {
      if (fileDialogActiveRef.current) {
        invoke("set_game_overlay_file_dialog_active", { active: false }).catch(console.error);
      }
    };
  }, []);

  function setFileDialogActive(active) {
    fileDialogActiveRef.current = active;
    invoke("set_game_overlay_file_dialog_active", { active }).catch(console.error);
  }

  function pickImages() {
    setFileDialogActive(true);
    const clearAfterFocus = () => {
      window.setTimeout(() => {
        if (fileDialogActiveRef.current) {
          setFileDialogActive(false);
        }
      }, 250);
    };
    window.addEventListener("focus", clearAfterFocus, { once: true });
    fileInputRef.current?.click();
  }

  function handleFileChange(event) {
    setFileDialogActive(false);
    const files = Array.from(event.target.files ?? []).filter((file) => file.type.startsWith("image/"));
    event.target.value = "";
    if (files.length === 0) return;

    Promise.all(
      files.map(
        (file) =>
          new Promise((resolve) => {
            const reader = new FileReader();
            reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : null);
            reader.onerror = () => resolve(null);
            reader.readAsDataURL(file);
          }),
      ),
    ).then((nextImages) => {
      onChange([...images, ...nextImages.filter(Boolean)]);
    });
  }

  function removeImage(index) {
    onChange(images.filter((_, itemIndex) => itemIndex !== index));
  }

  return (
    <div className="rounded border border-white/10 bg-black/20 p-3">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        onChange={handleFileChange}
        className="hidden"
      />
      <div className="mb-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-sm text-white/80">
          <ImageIcon className="h-4 w-4 text-white/45" />
          <span>{images.length > 0 ? `${images.length} images` : "No images selected"}</span>
        </div>
        <ResetButton onClick={onReset} />
      </div>
      {images.length > 0 ? (
        <div className="mb-3 grid grid-cols-3 gap-2">
          {images.map((src, index) => (
            <div key={`${src.slice(0, 32)}-${index}`} className="group relative overflow-hidden rounded border border-white/10 bg-black/35">
              <img src={src} alt="" className="aspect-square w-full object-contain" />
              <button
                type="button"
                className="absolute right-1 top-1 rounded bg-black/70 p-1 text-white/70 opacity-0 hover:text-white group-hover:opacity-100"
                onClick={() => removeImage(index)}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
        </div>
      ) : null}
      <Button type="button" variant="secondary" size="sm" className="h-9 w-full rounded" onClick={pickImages}>
        <Upload className="h-4 w-4" />
        Upload
      </Button>
    </div>
  );
}

export function ModuleSettings({ module, settings, onChange, onPreview, onReset }) {
  const [activeKeyInput, setActiveKeyInput] = useState(null);

  if (!module) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-white/45">
        Select a module.
      </div>
    );
  }

  if (module.settings.length === 0) {
    return <div className="text-sm text-white/45">This module has no settings.</div>;
  }

  return (
    <div className="space-y-4">
      {module.settings.map((item) => {
        const value = settingValue(settings, item);
        if (item.type === "boolean") {
          return (
            <label
              key={item.key}
              className="flex items-center justify-between gap-3 rounded border border-white/10 bg-black/20 px-3 py-2"
            >
              <span className="text-sm text-white/80">{item.label || item.key}</span>
              <div className="flex items-center gap-2">
                <ResetButton onClick={() => onReset(item)} />
                <input
                  type="checkbox"
                  checked={!!value}
                  onChange={(event) => onChange(item.key, event.target.checked)}
                  className="h-4 w-4 accent-[var(--theme-accent)]"
                />
              </div>
            </label>
          );
        }

        if (item.type === "color") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <input
                type="color"
                value={String(value)}
                onChange={(event) => onChange(item.key, event.target.value)}
                className="h-9 w-16 rounded border border-white/15 bg-transparent p-0"
              />
            </div>
          );
        }

        if (item.type === "range") {
          const numeric = Number(value);
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <span className="flex items-center gap-2">
                  <span className="tabular-nums text-white/40">{Number.isFinite(numeric) ? numeric : item.min}</span>
                  <ResetButton onClick={() => onReset(item)} />
                </span>
              </div>
              <Slider
                value={[Number.isFinite(numeric) ? numeric : Number(item.min ?? 0)]}
                min={Number(item.min ?? 0)}
                max={Number(item.max ?? 100)}
                step={Number(item.step ?? 1)}
                onValueChange={([next]) => onPreview(item.key, next)}
                onValueCommit={([next]) => onChange(item.key, next)}
              />
            </div>
          );
        }

        if (item.type === "number") {
          const numeric = Number(value);
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <input
                type="number"
                value={Number.isFinite(numeric) ? numeric : Number(item.default ?? item.min ?? 0)}
                min={item.min}
                max={item.max}
                step={item.step ?? 1}
                onChange={(event) => onChange(item.key, Number(event.target.value))}
                className="h-9 w-full rounded border border-white/10 bg-black/25 px-3 text-sm text-white outline-none"
              />
            </div>
          );
        }

        if (item.type === "select") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <select
                value={String(value ?? "")}
                onChange={(event) => onChange(item.key, event.target.value)}
                className="h-9 w-full rounded border border-white/10 bg-[#191b22] px-3 text-sm text-white outline-none"
              >
                {(item.options ?? []).map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label || option.value}
                  </option>
                ))}
              </select>
            </div>
          );
        }

        if (item.type === "textarea") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <ResetButton onClick={() => onReset(item)} />
              </div>
              <textarea
                value={String(value ?? "")}
                onChange={(event) => onChange(item.key, event.target.value)}
                rows={4}
                className="w-full resize-none rounded border border-white/10 bg-black/25 px-3 py-2 text-sm text-white outline-none"
              />
            </div>
          );
        }

        if (item.type === "key") {
          const isActive = activeKeyInput === item.key;
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
                <span className="flex items-center gap-2">
                  {isActive ? <span className="text-[var(--theme-accent)]">Listening...</span> : null}
                  <ResetButton onClick={() => onReset(item)} />
                </span>
              </div>
              <KeyCaptureButton
                active={isActive}
                value={value}
                onStart={() => setActiveKeyInput(item.key)}
                onCancel={() => setActiveKeyInput(null)}
                onCapture={(nextKey) => {
                  onChange(item.key, nextKey);
                  setActiveKeyInput(null);
                }}
              />
            </div>
          );
        }

        if (item.type === "image") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
              </div>
              <ImageSettingInput
                value={String(value ?? "")}
                onChange={(next) => onChange(item.key, next)}
                onReset={() => onReset(item)}
              />
            </div>
          );
        }

        if (item.type === "images") {
          return (
            <div key={item.key}>
              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
                <span>{item.label || item.key}</span>
              </div>
              <ImagesSettingInput
                value={value}
                onChange={(next) => onChange(item.key, next)}
                onReset={() => onReset(item)}
              />
            </div>
          );
        }

        return (
          <div key={item.key}>
            <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/55">
              <span>{item.label || item.key}</span>
              <ResetButton onClick={() => onReset(item)} />
            </div>
            <input
              value={String(value ?? "")}
              onChange={(event) => onChange(item.key, event.target.value)}
              className="h-9 w-full rounded border border-white/10 bg-black/25 px-3 text-sm text-white outline-none"
            />
          </div>
        );
      })}
    </div>
  );
}
