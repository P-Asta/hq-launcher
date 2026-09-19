import { cn } from "../../lib/cn";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useCallback, useEffect, useRef, useState } from "react";

export function ScrollableDropdownContent({
  className,
  scrollAreaClassName,
  children,
  ...props
}) {
  const scrollRef = useRef(null);
  const hideTimerRef = useRef(null);
  const [scrollState, setScrollState] = useState({
    visible: false,
    scrollable: false,
    thumbHeight: 0,
    thumbOffset: 0,
  });

  const showScrollbar = useCallback(() => {
    setScrollState((prev) => ({ ...prev, visible: true }));
    if (hideTimerRef.current) {
      window.clearTimeout(hideTimerRef.current);
    }
    hideTimerRef.current = window.setTimeout(() => {
      setScrollState((prev) => ({ ...prev, visible: false }));
      hideTimerRef.current = null;
    }, 500);
  }, []);

  const syncScrollbar = useCallback((shouldFlash = false) => {
    const el = scrollRef.current;
    if (!el) return;

    const scrollable = el.scrollHeight > el.clientHeight + 1;
    if (!scrollable) {
      setScrollState({
        visible: false,
        scrollable: false,
        thumbHeight: 0,
        thumbOffset: 0,
      });
      return;
    }

    const trackInset = 4;
    const trackHeight = Math.max(0, el.clientHeight - trackInset * 2);
    const thumbHeight = Math.max(
      24,
      Math.round((el.clientHeight / el.scrollHeight) * trackHeight),
    );
    const maxScroll = Math.max(1, el.scrollHeight - el.clientHeight);
    const maxOffset = Math.max(0, trackHeight - thumbHeight);
    const thumbOffset =
      trackInset + Math.round((el.scrollTop / maxScroll) * maxOffset);

    setScrollState((prev) => ({
      visible: shouldFlash ? true : prev.visible,
      scrollable: true,
      thumbHeight,
      thumbOffset,
    }));

    if (shouldFlash) {
      showScrollbar();
    }
  }, [showScrollbar]);

  useEffect(() => {
    syncScrollbar(true);
    const el = scrollRef.current;
    if (!el || typeof ResizeObserver === "undefined") {
      return () => {
        if (hideTimerRef.current) {
          window.clearTimeout(hideTimerRef.current);
          hideTimerRef.current = null;
        }
      };
    }

    const observer = new ResizeObserver(() => {
      syncScrollbar(false);
    });
    observer.observe(el);

    return () => {
      observer.disconnect();
      if (hideTimerRef.current) {
        window.clearTimeout(hideTimerRef.current);
        hideTimerRef.current = null;
      }
    };
  }, [syncScrollbar]);

  return (
    <DropdownMenu.Content
      className={cn(
        "relative overflow-hidden",
        className,
      )}
      {...props}
    >
      <div
        ref={scrollRef}
        onScroll={() => syncScrollbar(true)}
        className={cn(
          "dropdown-scroll-area overflow-y-auto",
          scrollAreaClassName,
        )}
      >
        {children}
      </div>
      {scrollState.scrollable ? (
        <div
          className={cn(
            "pointer-events-none absolute bottom-1 right-1 top-1 w-1 transition-opacity duration-150",
            scrollState.visible ? "opacity-100" : "opacity-0",
          )}
        >
          <div
            className="absolute right-0 w-1 rounded-full bg-white/35"
            style={{
              height: `${scrollState.thumbHeight}px`,
              transform: `translateY(${scrollState.thumbOffset}px)`,
            }}
          />
        </div>
      ) : null}
    </DropdownMenu.Content>
  );
}
