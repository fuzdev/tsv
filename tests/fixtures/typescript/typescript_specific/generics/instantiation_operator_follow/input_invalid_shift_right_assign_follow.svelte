<script lang="ts">
	// A `>>=` is `>`-led, so it disqualifies the list whatever it is — an assignment
	// operator included. tsc's grammar refuses the list ahead of any `>` (its re-scan cannot
	// tell the close from a longer operator), which leaves `f < T >` with no operand.
	// acorn-typescript rejects too, but by its own reading: an instantiation target, which
	// its assignment-target early error refuses (and which it skips inside a paren or a call
	// argument list, where it accepts).
	f<T> >>= c;
</script>
