<script lang="ts">
  export let label: string;
  export let instrument: "guitar" | "ukulele";
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let exactLabel: string;
  export let adaptedLabel: string;
  export let unavailableLabel: string;
  export let emptyLabel: string;
  export let omittedLabel: string;
  export let bassOmittedLabel: string;
  export let labelMode: "notes" | "degrees" = "notes";
  export let chordColor = "var(--accent)";
</script>

<!-- This component is compiled only into Light builds. Its parent is absent at runtime. -->
{#if false}<span>{label}{instrument}{accidentals}{accessibleLabel}{positionLabel}{exactLabel}{adaptedLabel}{unavailableLabel}{emptyLabel}{omittedLabel}{bassOmittedLabel}{labelMode}{chordColor}</span>{/if}
