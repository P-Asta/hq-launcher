import { useEffect, useState } from "react";

export function ModCover({ src, initials, onMissing }) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [src]);

  if (src && !failed) {
    return (
      <div className="flex h-14 w-14 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-panel-outline bg-black/25">
        <img
          src={src}
          alt=""
          className="h-full w-full object-contain"
          loading="lazy"
          onError={() => {
            console.warn("Failed to load mod icon:", src);
            if (typeof onMissing === "function") {
              onMissing(src);
            }
            setFailed(true);
          }}
        />
      </div>
    );
  }

  return (
    <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-panel-outline bg-white/10 text-base font-bold text-white/80">
      {initials}
    </div>
  );
}
