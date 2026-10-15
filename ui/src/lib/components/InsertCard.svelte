<script lang="ts">
  /** Animated "insert card" illustration. `variant="noReader"` shows an unplugged reader. */
  let { variant = "insert" }: { variant?: "insert" | "noReader" } = $props();
</script>

<svg class="art {variant}" viewBox="0 0 260 210" role="img" aria-hidden="true">
  <defs>
    <linearGradient id="ic-card" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#4a76ff" />
      <stop offset="1" stop-color="#8a5cff" />
    </linearGradient>
    <linearGradient id="ic-chip" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffe7a3" />
      <stop offset="1" stop-color="#eeb23a" />
    </linearGradient>
    <linearGradient id="ic-reader" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" class="rd-top" />
      <stop offset="1" class="rd-bottom" />
    </linearGradient>
    <clipPath id="ic-clip">
      <rect x="0" y="0" width="260" height="128" />
    </clipPath>
  </defs>

  <!-- glow -->
  <ellipse class="glow" cx="130" cy="120" rx="110" ry="70" />

  {#if variant === "insert"}
    <!-- card sliding into the slot -->
    <g clip-path="url(#ic-clip)">
      <g class="card">
        <rect x="82" y="14" width="96" height="138" rx="10" fill="url(#ic-card)" />
        <rect x="82" y="14" width="96" height="138" rx="10" fill="#fff" opacity="0.08" />
        <rect x="96" y="32" width="26" height="20" rx="4" fill="url(#ic-chip)" />
        <path d="M96 42h26M109 32v20M103 32v6M115 46v6" stroke="#c78a14" stroke-width="1" fill="none" />
        <circle cx="152" cy="44" r="11" fill="#fff" opacity="0.28" />
        <rect x="96" y="66" width="68" height="6" rx="3" fill="#fff" opacity="0.55" />
        <rect x="96" y="80" width="48" height="6" rx="3" fill="#fff" opacity="0.35" />
        <rect x="96" y="94" width="58" height="6" rx="3" fill="#fff" opacity="0.35" />
        <path class="shine" d="M70 150 L120 0 L140 0 L90 150 Z" fill="#fff" opacity="0.18" />
      </g>
    </g>
  {:else}
    <!-- cable + unplugged connector -->
    <path class="cable" d="M206 150 C 236 150, 236 80, 210 64 S 168 40, 176 20" fill="none" stroke-width="5" stroke-linecap="round" />
    <g class="plug">
      <rect x="166" y="2" width="20" height="22" rx="4" class="plug-body" />
      <rect x="170" y="-6" width="4" height="9" rx="1" class="plug-pin" />
      <rect x="178" y="-6" width="4" height="9" rx="1" class="plug-pin" />
    </g>
  {/if}

  <!-- reader -->
  <rect x="40" y="122" width="180" height="62" rx="16" fill="url(#ic-reader)" class="reader" />
  <rect x="72" y="124" width="116" height="7" rx="3.5" class="slot" />
  <circle cx="196" cy="160" r="5" class="led" />
  <rect x="62" y="156" width="54" height="6" rx="3" class="detail" />
  <ellipse cx="130" cy="196" rx="84" ry="7" class="shadow" />
</svg>

<style>
  .art {
    width: 240px;
    max-width: 70%;
    height: auto;
    overflow: visible;
  }
  .glow {
    fill: var(--accent-soft);
  }
  .rd-top {
    stop-color: #3a4256;
  }
  .rd-bottom {
    stop-color: #232a3a;
  }
  :global([data-theme="dark"]) .rd-top {
    stop-color: #2b3242;
  }
  :global([data-theme="dark"]) .rd-bottom {
    stop-color: #1a1f2b;
  }
  .reader {
    stroke: rgba(255, 255, 255, 0.06);
  }
  .slot {
    fill: #0b0e15;
  }
  .detail {
    fill: rgba(255, 255, 255, 0.12);
  }
  .shadow {
    fill: rgba(0, 0, 0, 0.12);
  }
  .led {
    fill: #f2a33a;
    animation: blink 1.6s ease-in-out infinite;
  }
  .noReader .led {
    fill: #6d7688;
    animation: none;
  }
  .card {
    animation: insert 3.2s var(--ease) infinite;
  }
  .shine {
    animation: shine 3.2s ease-in-out infinite;
  }
  .cable {
    stroke: #4a5266;
  }
  .plug-body {
    fill: #4a5266;
  }
  .plug-pin {
    fill: #b8bfcc;
  }
  .plug {
    animation: float 3s ease-in-out infinite;
  }
  @keyframes insert {
    0% {
      transform: translateY(-14px);
      opacity: 0;
    }
    12% {
      opacity: 1;
    }
    55%,
    72% {
      transform: translateY(56px);
      opacity: 1;
    }
    90%,
    100% {
      transform: translateY(56px);
      opacity: 0;
    }
  }
  @keyframes shine {
    0%,
    30% {
      transform: translateX(-60px);
    }
    70%,
    100% {
      transform: translateX(120px);
    }
  }
  @keyframes blink {
    0%,
    100% {
      opacity: 0.35;
    }
    50% {
      opacity: 1;
    }
  }
  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-5px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .card {
      animation: none;
      transform: translateY(30px);
    }
  }
</style>
