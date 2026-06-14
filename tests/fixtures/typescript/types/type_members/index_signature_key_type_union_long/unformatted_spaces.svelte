<script lang="ts">
	// Union key at exactly 100 cols stays fully inline (no wrap)
	interface A {
		[  k  :   AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB  ]: boolean;
	}

	// At 101 cols the bracket breaks; the key and union move to one indented line
	interface B {
		[  k  :   AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB  ]: boolean;
	}

	// A union too wide to fit even after the bracket breaks wraps onto leading pipes
	interface C {
		[  k  :   AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB  ]: boolean;
	}
</script>
