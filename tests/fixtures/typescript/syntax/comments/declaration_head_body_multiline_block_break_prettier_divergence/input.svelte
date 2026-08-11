<script lang="ts">
	// A multiline block the author broke after keeps the break: the body `{` drops
	// to its own line, flush with the head.
	function a() /* x
y */
	{
		fn();
	}

	// Class method - prettier relocates this one before the parameter list.
	class A {
		b() /* x
y */
		{
			fn();
		}
	}

	// Class body brace.
	class B /* x
y */
	{
		x = 1;
	}

	// Interface body brace.
	interface C /* x
y */
	{
		x: number;
	}

	// Enum body brace.
	enum D /* x
y */
	{
		A,
		B
	}

	// Namespace body brace.
	namespace E /* x
y */
	{
		export const x = 1;
	}
</script>
