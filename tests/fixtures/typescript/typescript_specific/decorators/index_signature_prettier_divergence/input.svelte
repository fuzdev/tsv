<script lang="ts">
	// a decorator list on a class index signature is kept and printed, inline or own-line as
	// authored, like any member's
	class A {
		@a [k: string]: T;

		@b
		@c(d)
		[k: number]: T;
	}
</script>
