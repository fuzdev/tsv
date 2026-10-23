<script lang="ts">
	// an assignment a compound left holds, as a property value or an element, keeps its own
	// operator
	({ a: (b += c) } += x);
	f(([(a += b)] += x));
	f(([a = 1] += x));
	({
		a: [(b ||= c)]
	} -= x);
</script>
