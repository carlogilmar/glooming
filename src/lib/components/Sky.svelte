<script lang="ts">
  // Weather in the masthead.
  //
  // Home is the one surface in the app where ambient motion is affordable: you
  // are not reading code on it, nothing here has to be tracked, and it is the
  // only screen with room for the app to have a personality. Everywhere else the
  // rule holds — motion carries information, and the budget for infinite motion
  // is spent on the focus pulses and the treemap.
  //
  // Three weathers, and **which one you get is decided when home appears**. It is
  // the same argument a screensaver makes: a surface you land on a dozen times a
  // day should not be identical a dozen times a day, and none of the three says
  // anything, so none of them can say the wrong thing by being picked.
  //
  // Mocked in `mockup/home.html`, which keeps all four candidates (Constellation
  // was cut: it draws lines between dots, and the reach diagram is a real graph —
  // teaching the eye that those are ornamental is the one thing to avoid).

  import { theme } from "$lib/stores/theme.svelte";

  type Mode = "bloom" | "cosmos" | "aurora";
  const MODES: Mode[] = ["bloom", "cosmos", "aurora"];

  let canvas = $state<HTMLCanvasElement | null>(null);
  /** Rolled once per mount — landing on home again gives you another. */
  const mode: Mode = MODES[Math.floor(Math.random() * MODES.length)];

  $effect(() => {
    // Re-run when the theme flips: the accent is read from the token, once, and
    // reading it every frame to catch a change nobody makes mid-frame is waste.
    theme.resolved;

    const el = canvas;
    const ctx = el?.getContext("2d");
    if (!el || !ctx) return;

    const host = el.parentElement;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const rgb = accentRgb(el);

    let w = 0;
    let h = 0;
    let raf: number | null = null;
    let last = 0;
    let clock = 0;

    let rings: { x: number; y: number; t: number; period: number }[] = [];
    let stars: { x: number; y: number; z: number; ph: number }[] = [];

    const teal = (a: number) => `rgba(${rgb}, ${a})`;

    function seed() {
      // Bloom: the name's own origin — the swell when water first hits coffee
      // grounds. Staggered periods, so they never pulse in unison.
      rings = Array.from({ length: 5 }, (_, i) => ({
        x: w * (0.12 + 0.19 * i) + (i % 2 ? 40 : -20),
        y: h * (i % 2 ? 0.34 : 0.66),
        t: i * 0.9,
        period: 7 + i * 1.3,
      }));
      stars = Array.from({ length: 70 }, () => ({
        x: Math.random() * w,
        y: Math.random() * h,
        z: 0.3 + Math.random() * 0.7,
        ph: Math.random() * Math.PI * 2,
      }));
    }

    function fit() {
      const r = host?.getBoundingClientRect();
      if (!r || !el || !ctx) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = r.width;
      h = r.height;
      el.width = w * dpr;
      el.height = h * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      seed();
    }

    function bloom(dt: number) {
      for (const r of rings) {
        r.t += dt;
        const p = (r.t % r.period) / r.period;
        ctx!.beginPath();
        ctx!.arc(r.x, r.y, 8 + p * Math.min(w, h) * 0.55, 0, Math.PI * 2);
        ctx!.strokeStyle = teal(0.3 * (1 - p) ** 1.6);
        ctx!.lineWidth = 1 + (1 - p) * 1.6;
        ctx!.stroke();
        ctx!.beginPath();
        ctx!.arc(r.x, r.y, 2.2, 0, Math.PI * 2);
        ctx!.fillStyle = teal(0.5);
        ctx!.fill();
      }
    }

    function cosmos(dt: number) {
      for (const s of stars) {
        s.x += dt * 6 * s.z;
        if (s.x > w + 2) s.x = -2;
        s.ph += dt * 1.2;
        ctx!.beginPath();
        ctx!.arc(s.x, s.y, s.z * 1.5, 0, Math.PI * 2);
        ctx!.fillStyle = teal((0.18 + 0.22 * s.z) * (0.65 + 0.35 * Math.sin(s.ph)));
        ctx!.fill();
      }
    }

    function aurora(t: number) {
      for (let b = 0; b < 3; b++) {
        const amp = 14 + b * 9;
        const y0 = h * (0.42 + b * 0.14);
        ctx!.beginPath();
        ctx!.moveTo(0, h);
        for (let x = 0; x <= w; x += 12) {
          ctx!.lineTo(x, y0 + Math.sin(x / (140 + b * 60) + t * (0.16 + b * 0.07)) * amp);
        }
        ctx!.lineTo(w, h);
        ctx!.closePath();
        const g = ctx!.createLinearGradient(0, y0 - amp, 0, h);
        g.addColorStop(0, teal(0.16 - b * 0.04));
        g.addColorStop(1, teal(0));
        ctx!.fillStyle = g;
        ctx!.fill();
      }
    }

    function frame(now: number) {
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      clock += dt;
      ctx!.clearRect(0, 0, w, h);
      if (mode === "bloom") bloom(dt);
      else if (mode === "cosmos") cosmos(dt);
      else aurora(clock);
      if (!reduced) raf = requestAnimationFrame(frame);
    }

    function start() {
      stop();
      last = performance.now();
      // Reduced motion composes ONE frame and holds it: the picture survives, the
      // movement does not. `clock` is nudged forward so aurora is drawn mid-wave
      // rather than as three flat lines.
      if (reduced) {
        clock = 3;
        frame(last + 16);
        return;
      }
      raf = requestAnimationFrame(frame);
    }

    function stop() {
      if (raf !== null) cancelAnimationFrame(raf);
      raf = null;
    }

    // Nothing animates in a window you are not looking at.
    const onVisible = () => (document.hidden ? stop() : start());

    const ro = new ResizeObserver(() => {
      fit();
      start();
    });
    if (host) ro.observe(host);
    document.addEventListener("visibilitychange", onVisible);

    fit();
    start();

    return () => {
      stop();
      ro.disconnect();
      document.removeEventListener("visibilitychange", onVisible);
    };
  });

  /** `--gloom` as "r, g, b", read once from the live token. */
  function accentRgb(el: HTMLElement): string {
    const hex = getComputedStyle(el).getPropertyValue("--gloom").trim();
    const n = parseInt(hex.replace("#", ""), 16);
    return Number.isNaN(n) ? "95, 214, 196" : `${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}`;
  }
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
</style>
