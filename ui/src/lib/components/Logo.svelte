<script lang="ts">
  let { size = 34 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 40 40" aria-hidden="true">
  <defs>
    <linearGradient id="logo-g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#3d6bff" />
      <stop offset="1" stop-color="#7a4cff" />
    </linearGradient>
    <linearGradient id="logo-chip" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffe59a" />
      <stop offset="1" stop-color="#f2b63c" />
    </linearGradient>
  </defs>
  <rect width="40" height="40" rx="11" fill="url(#logo-g)" />
  <rect x="8" y="11" width="24" height="18" rx="3.2" fill="#fff" fill-opacity="0.96" />
  <rect x="11.5" y="15" width="7" height="5.6" rx="1.3" fill="url(#logo-chip)" />
  <path d="M11.5 17.8h7M15 15v5.6" stroke="#c98a12" stroke-width="0.7" />
  <rect x="21.5" y="15.4" width="7" height="1.8" rx="0.9" fill="#3d6bff" fill-opacity="0.55" />
  <rect x="21.5" y="19" width="4.6" height="1.8" rx="0.9" fill="#3d6bff" fill-opacity="0.35" />
  <rect x="11.5" y="23.6" width="17" height="1.8" rx="0.9" fill="#7a4cff" fill-opacity="0.3" />
</svg>
