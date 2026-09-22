import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { CheckCircle, X } from "@phosphor-icons/react";
import { FrostFloat } from "./FrostFloat";

type Celebration = {
  title: string;
};

type GoalCompleteContextValue = {
  celebrateGoalComplete: (title: string) => void;
};

const GoalCompleteContext = createContext<GoalCompleteContextValue | null>(null);

export function useGoalComplete() {
  const ctx = useContext(GoalCompleteContext);
  if (!ctx) throw new Error("useGoalComplete must be used within GoalCompleteProvider");
  return ctx;
}

type Particle = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  rot: number;
  vr: number;
  w: number;
  h: number;
  color: string;
  life: number;
};

const COLORS = ["#e8c56b", "#eaf6cb", "#9dde9a", "#f0d98a", "#c5d9a0", "#ffe9a8"];

function spawnBurst(cx: number, cy: number, count = 90): Particle[] {
  const out: Particle[] = [];
  for (let i = 0; i < count; i++) {
    const angle = Math.random() * Math.PI * 2;
    const speed = 3.5 + Math.random() * 7;
    out.push({
      x: cx,
      y: cy,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed - 4,
      rot: Math.random() * 360,
      vr: (Math.random() - 0.5) * 14,
      w: 5 + Math.random() * 6,
      h: 3 + Math.random() * 4,
      color: COLORS[i % COLORS.length],
      life: 1,
    });
  }
  return out;
}

function ConfettiCanvas({ active }: { active: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const particlesRef = useRef<Particle[]>([]);
  const rafRef = useRef<number>(0);

  useEffect(() => {
    if (!active) {
      particlesRef.current = [];
      cancelAnimationFrame(rafRef.current);
      const c = canvasRef.current;
      if (c) {
        const ctx = c.getContext("2d");
        ctx?.clearRect(0, 0, c.width, c.height);
      }
      return;
    }

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const resize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    resize();
    window.addEventListener("resize", resize);

    particlesRef.current = spawnBurst(canvas.width / 2, canvas.height * 0.42);
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const tick = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      if (reduced) return;
      const next: Particle[] = [];
      for (const p of particlesRef.current) {
        p.vy += 0.18;
        p.vx *= 0.992;
        p.x += p.vx;
        p.y += p.vy;
        p.rot += p.vr;
        p.life -= 0.008;
        if (p.life <= 0 || p.y > canvas.height + 40) continue;
        ctx.save();
        ctx.translate(p.x, p.y);
        ctx.rotate((p.rot * Math.PI) / 180);
        ctx.globalAlpha = Math.max(0, p.life);
        ctx.fillStyle = p.color;
        ctx.fillRect(-p.w / 2, -p.h / 2, p.w, p.h);
        ctx.restore();
        next.push(p);
      }
      particlesRef.current = next;
      if (next.length) rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);

    return () => {
      window.removeEventListener("resize", resize);
      cancelAnimationFrame(rafRef.current);
    };
  }, [active]);

  return (
    <canvas
      ref={canvasRef}
      className="pointer-events-none absolute inset-0 h-full w-full"
      aria-hidden
    />
  );
}

function GoalCompleteCard({
  celebration,
  onClose,
}: {
  celebration: Celebration;
  onClose: () => void;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-100 grid place-items-center bg-overlay p-4"
      onClick={onClose}
      role="presentation"
    >
      <ConfettiCanvas active />
      <FrostFloat
        className="relative z-10 w-full max-w-sm p-6 text-center"
        role="dialog"
        aria-modal="true"
        aria-labelledby="goal-complete-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="absolute top-2.5 right-2.5">
          <button
            ref={closeRef}
            type="button"
            aria-label="Close"
            title="Close"
            onClick={onClose}
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg text-muted hover:bg-raised-soft hover:text-ink"
          >
            <X size={16} weight="bold" />
          </button>
        </div>
        <div className="mx-auto mb-3 grid size-12 place-items-center rounded-full bg-warn/20 text-warn">
          <CheckCircle size={28} weight="fill" />
        </div>
        <p className="m-0 text-xs tracking-wide text-warn uppercase">Goal reached</p>
        <h2 id="goal-complete-title" className="mt-2 mb-0 font-display text-2xl font-medium text-ink">
          You did it
        </h2>
        <p className="mt-2 mb-0 text-sm text-ink-soft">
          <span className="font-medium text-warn">{celebration.title}</span> is complete. Nice work.
        </p>
      </FrostFloat>
    </div>
  );
}

export function GoalCompleteProvider({ children }: { children: ReactNode }) {
  const [celebration, setCelebration] = useState<Celebration | null>(null);

  const celebrateGoalComplete = useCallback((title: string) => {
    const trimmed = title.trim() || "Your goal";
    setCelebration({ title: trimmed });
  }, []);

  const value = useMemo(() => ({ celebrateGoalComplete }), [celebrateGoalComplete]);

  return (
    <GoalCompleteContext.Provider value={value}>
      {children}
      {celebration && (
        <GoalCompleteCard celebration={celebration} onClose={() => setCelebration(null)} />
      )}
    </GoalCompleteContext.Provider>
  );
}
