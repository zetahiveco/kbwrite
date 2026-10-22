import { useEffect, useRef } from "react";
import type { GraphData } from "../types";

const REPULSION = 120_000;
const SPRING_K = 0.045;
const REST_LEN = 120;
const CENTER_PULL = 0.008;
const DAMPING = 0.82;
const STEPS = 3;
const MAX_SPEED = 24;

type Sim = {
  ids: string[];
  titles: string[];
  pos: { x: number; y: number }[];
  vel: { x: number; y: number }[];
  edges: [number, number][];
  degree: number[];
  offsetX: number;
  offsetY: number;
  zoom: number;
};

function seed(i: number) {
  const angle = i * 2.399963 + 1;
  const radius = 30 * Math.sqrt(i + 1);
  return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
}

export function GraphView({
  data,
  currentPath,
  onOpen,
}: {
  data: GraphData;
  currentPath: string | null;
  onOpen: (path: string) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const simRef = useRef<Sim | null>(null);
  const currentRef = useRef(currentPath);
  const onOpenRef = useRef(onOpen);
  currentRef.current = currentPath;
  onOpenRef.current = onOpen;

  useEffect(() => {
    const prev = simRef.current;
    const old = new Map<string, { x: number; y: number }>();
    prev?.ids.forEach((id, index) => old.set(id, prev.pos[index]));
    const degree = data.nodes.map(() => 0);
    for (const [a, b] of data.edges) {
      degree[a] += 1;
      degree[b] += 1;
    }
    simRef.current = {
      ids: data.nodes.map((node) => node.path),
      titles: data.nodes.map((node) => node.title),
      pos: data.nodes.map((node, index) => old.get(node.path) ?? seed(index)),
      vel: data.nodes.map(() => ({ x: 0, y: 0 })),
      edges: data.edges,
      degree,
      offsetX: prev?.offsetX ?? 0,
      offsetY: prev?.offsetY ?? 0,
      zoom: prev?.zoom ?? 1,
    };
  }, [data]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let frame = 0;
    const drag = { active: false, moved: false, x: 0, y: 0 };

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const width = wrap.clientWidth;
      const height = wrap.clientHeight;
      canvas.width = Math.max(1, Math.floor(width * dpr));
      canvas.height = Math.max(1, Math.floor(height * dpr));
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(wrap);

    const step = (sim: Sim) => {
      const count = sim.pos.length;
      if (count === 0) return;
      for (let pass = 0; pass < STEPS; pass++) {
        const fx = new Array<number>(count).fill(0);
        const fy = new Array<number>(count).fill(0);
        for (let i = 0; i < count; i++) {
          for (let j = i + 1; j < count; j++) {
            let dx = sim.pos[i].x - sim.pos[j].x;
            let dy = sim.pos[i].y - sim.pos[j].y;
            let dist = Math.hypot(dx, dy);
            if (dist < 0.01) {
              dx = 0.01;
              dy = 0;
              dist = 0.01;
            }
            const force = REPULSION / (dist * dist);
            const ux = (dx / dist) * force;
            const uy = (dy / dist) * force;
            fx[i] += ux;
            fy[i] += uy;
            fx[j] -= ux;
            fy[j] -= uy;
          }
          fx[i] -= sim.pos[i].x * CENTER_PULL;
          fy[i] -= sim.pos[i].y * CENTER_PULL;
        }
        for (const [a, b] of sim.edges) {
          const dx = sim.pos[b].x - sim.pos[a].x;
          const dy = sim.pos[b].y - sim.pos[a].y;
          const dist = Math.hypot(dx, dy) || 0.01;
          const force = SPRING_K * (dist - REST_LEN);
          const ux = (dx / dist) * force;
          const uy = (dy / dist) * force;
          fx[a] += ux;
          fy[a] += uy;
          fx[b] -= ux;
          fy[b] -= uy;
        }
        for (let i = 0; i < count; i++) {
          sim.vel[i].x = (sim.vel[i].x + fx[i]) * DAMPING;
          sim.vel[i].y = (sim.vel[i].y + fy[i]) * DAMPING;
          const speed = Math.hypot(sim.vel[i].x, sim.vel[i].y);
          if (speed > MAX_SPEED) {
            sim.vel[i].x = (sim.vel[i].x / speed) * MAX_SPEED;
            sim.vel[i].y = (sim.vel[i].y / speed) * MAX_SPEED;
          }
          sim.pos[i].x += sim.vel[i].x;
          sim.pos[i].y += sim.vel[i].y;
        }
      }
    };

    const draw = () => {
      const sim = simRef.current;
      const width = wrap.clientWidth;
      const height = wrap.clientHeight;
      ctx.clearRect(0, 0, width, height);
      if (!sim) return;
      const style = getComputedStyle(wrap);
      const edge = style.getPropertyValue("--graph-edge").trim();
      const node = style.getPropertyValue("--graph-node").trim();
      const accent = style.getPropertyValue("--accent").trim();
      const label = style.getPropertyValue("--muted").trim();
      const cx = width / 2 + sim.offsetX;
      const cy = height / 2 + sim.offsetY;
      const toScreen = (point: { x: number; y: number }) => ({
        x: cx + point.x * sim.zoom,
        y: cy + point.y * sim.zoom,
      });

      ctx.lineWidth = 1;
      ctx.strokeStyle = edge;
      for (const [a, b] of sim.edges) {
        const start = toScreen(sim.pos[a]);
        const end = toScreen(sim.pos[b]);
        ctx.beginPath();
        ctx.moveTo(start.x, start.y);
        ctx.lineTo(end.x, end.y);
        ctx.stroke();
      }

      ctx.font = "12px -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "top";
      sim.pos.forEach((point, index) => {
        const screen = toScreen(point);
        const hot = sim.ids[index] === currentRef.current;
        const radius = Math.min(4.5 + sim.degree[index] * 0.9, 14) * Math.max(sim.zoom, 0.5);
        ctx.beginPath();
        ctx.fillStyle = hot ? accent : node;
        ctx.arc(screen.x, screen.y, radius, 0, Math.PI * 2);
        ctx.fill();
        if (sim.zoom >= 0.55 || hot) {
          ctx.fillStyle = label;
          ctx.fillText(sim.titles[index], screen.x, screen.y + radius + 6);
        }
      });

      if (sim.ids.length === 0) {
        ctx.fillStyle = label;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.font = "14px -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif";
        ctx.fillText(
          "No notes to graph yet. Link notes with [[wikilinks]].",
          width / 2,
          height / 2,
        );
      }
    };

    const hit = (x: number, y: number) => {
      const sim = simRef.current;
      if (!sim) return -1;
      const cx = wrap.clientWidth / 2 + sim.offsetX;
      const cy = wrap.clientHeight / 2 + sim.offsetY;
      let best = -1;
      let bestDistance = 16;
      sim.pos.forEach((point, index) => {
        const distance = Math.hypot(cx + point.x * sim.zoom - x, cy + point.y * sim.zoom - y);
        if (distance < bestDistance) {
          bestDistance = distance;
          best = index;
        }
      });
      return best;
    };

    const loop = () => {
      if (simRef.current) step(simRef.current);
      draw();
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);

    const onPointerDown = (event: PointerEvent) => {
      drag.active = true;
      drag.moved = false;
      drag.x = event.clientX;
      drag.y = event.clientY;
      canvas.setPointerCapture(event.pointerId);
    };
    const onPointerMove = (event: PointerEvent) => {
      if (!drag.active || !simRef.current) return;
      const dx = event.clientX - drag.x;
      const dy = event.clientY - drag.y;
      if (Math.hypot(dx, dy) > 3) drag.moved = true;
      simRef.current.offsetX += dx;
      simRef.current.offsetY += dy;
      drag.x = event.clientX;
      drag.y = event.clientY;
    };
    const onPointerUp = (event: PointerEvent) => {
      if (!drag.active) return;
      drag.active = false;
      if (drag.moved) return;
      const rect = canvas.getBoundingClientRect();
      const index = hit(event.clientX - rect.left, event.clientY - rect.top);
      const sim = simRef.current;
      if (index >= 0 && sim) onOpenRef.current(sim.ids[index]);
    };
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const sim = simRef.current;
      if (!sim) return;
      const rect = canvas.getBoundingClientRect();
      const px = event.clientX - rect.left;
      const py = event.clientY - rect.top;
      const width = wrap.clientWidth;
      const height = wrap.clientHeight;
      const next = Math.min(4, Math.max(0.15, sim.zoom * (1 - event.deltaY * 0.0015)));
      const gx = (px - (width / 2 + sim.offsetX)) / sim.zoom;
      const gy = (py - (height / 2 + sim.offsetY)) / sim.zoom;
      sim.zoom = next;
      sim.offsetX = px - width / 2 - gx * next;
      sim.offsetY = py - height / 2 - gy * next;
    };

    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerup", onPointerUp);
    canvas.addEventListener("wheel", onWheel, { passive: false });

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("wheel", onWheel);
    };
  }, []);

  return (
    <div className="graph-wrap" ref={wrapRef}>
      <canvas ref={canvasRef} className="graph-canvas" />
    </div>
  );
}
