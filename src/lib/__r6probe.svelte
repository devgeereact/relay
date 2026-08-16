<!-- R6 audit fixture. Not shipped UI, not routed, not reachable. -->
<script>
  import { onMount, afterUpdate, onDestroy } from 'svelte';
  export let seen = [];
  onMount(() => seen.push('mount'));
  afterUpdate(() => seen.push('afterUpdate'));
  onDestroy(() => seen.push('destroy'));
</script>

<p>probe</p>
