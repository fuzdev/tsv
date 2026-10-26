<script lang="ts">
	// a decorator list on a static block is kept and printed, inline or own-line as
	// authored, like any member's
	class A {
		static {}

		static {
			x();
		}
	}
</script>
