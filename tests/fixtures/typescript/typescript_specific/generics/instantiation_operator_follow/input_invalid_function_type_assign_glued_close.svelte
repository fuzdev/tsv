<script lang="ts">
	// A function-type head commits its list before the follower is read, and a close glued
	// to `=` is no close: tsc re-scans it as `>=`, leaving nothing to read `() => U` as.
	f<() => U>=c;
</script>
