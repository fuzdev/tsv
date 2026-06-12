<script lang="ts">
	// Simple qualified name
	type A = ns.Type;

	// Deeply nested
	type B = a.b.c.d.E;

	// In union/intersection
	type C = ns.A | ns.B;
	type D = ns.A & ns.B;

	// In generics
	type E = ns.Box<ns.Item>;

	// As return type
	function fn(): ns.Result {
		return {} as ns.Result;
	}

	// In extends
	interface F extends ns.Base {}
</script>
