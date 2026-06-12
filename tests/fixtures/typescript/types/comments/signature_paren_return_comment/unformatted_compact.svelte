<script lang="ts">
	// type literal method signature (reference - already correct)
	type T = {m(a)/* c */:void};

	// interface method signature
	interface I1 {m(a)/* c */:void;}

	// interface call signature
	interface I2 {(a)/* c */:void;}

	// interface construct signature
	interface I3 {new (a)/* c */:void;}

	// declare function
	declare function fn(a)/* c */:void;
</script>
