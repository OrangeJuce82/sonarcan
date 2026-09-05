<script lang="ts">
  export let label: string;
  export let accidentals: "flat" | "sharp";
  export let accessibleLabel: string;
  export let positionLabel: string;
  export let unavailableLabel: string;
  export let emptyLabel: string;
  export let labelMode: "notes" | "degrees" = "notes";
  export let chordColor = "var(--accent)";
</script>

<!-- This component is compiled only into Light builds. Its parent is absent at runtime. -->
{#if false}<span>{label}{accidentals}{accessibleLabel}{positionLabel}{unavailableLabel}{emptyLabel}{labelMode}{chordColor}</span>{/if}
