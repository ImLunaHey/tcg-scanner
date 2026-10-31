import { useEffect, useRef, type ReactNode } from "react";

export function Modal({
  close,
  children,
}: {
  close: () => void;
  children: ReactNode;
}) {
  const root = useRef<HTMLDivElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    document.body.classList.add("modal-open");
    return () => {
      document.body.classList.remove("modal-open");
      if (previous?.isConnected) previous.focus();
    };
  }, []);
  return (
    <div
      ref={root}
      className="modal-backdrop"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          closeRef.current();
          return;
        }
        if (event.key !== "Tab") return;
        const elements = Array.from(
          root.current?.querySelectorAll<HTMLElement>(
            'button:not(:disabled), input:not(:disabled), select:not(:disabled), a[href], [tabindex="0"]',
          ) || [],
        ).filter(
          (element) =>
            element.getClientRects().length > 0 &&
            !element.closest("fieldset:disabled"),
        );
        const first = elements[0];
        const last = elements.at(-1);
        if (!first) {
          event.preventDefault();
          return;
        }
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }}
    >
      {children}
    </div>
  );
}
