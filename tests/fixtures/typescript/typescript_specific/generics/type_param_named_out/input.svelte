<script lang="ts">
	// `out` is contextual: with no name able to follow it, it IS the name.
	type A<out> = out;
	type B<out, T> = [out, T];
	type C<out = string> = out;
	// A name after it makes it the variance modifier again — including its own
	// spelling, which is the whole discrimination: the token AFTER decides.
	interface D<out T> {
		value: T;
	}
	type E<out out> = out;
	// `in` is reserved, so the second word can only be the name.
	type G<in out> = number;
	class F<out> {}
	function fn<out>(value: out): out {
		return value;
	}
</script>
