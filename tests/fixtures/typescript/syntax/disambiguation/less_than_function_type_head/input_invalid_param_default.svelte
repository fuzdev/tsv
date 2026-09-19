<script lang="ts">
	// A parameter DEFAULT is a parameter start's follower, so the group is a function
	// type's parameter list — but no function type carries a default, and the canonical
	// parser refuses the type outright.
	f<(a = x) => U>(x);
</script>
