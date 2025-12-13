<script lang="ts">
	// Numeric literals
	type Num = 1;
	type Float = 3.14;
	type Hex = 0xff;
	type Bin = 0b1010;
	type Oct = 0o777;
	type Sci = 1e10;
	type Sep = 1_000_000;

	// String literals
	type Str = 'a';
	type Quote = "it's";

	// BigInt literals
	type Big = 1n;
	type BigHex = 0xffn;

	// Negative literals
	type Neg = -1;
	type NegBig = -1n;

	// Boolean literals
	type T = true;
	type F = false;

	// Literal types in unions
	type Union = 'a' | 'b' | 'c';
	type Digits = 0 | 1 | 2;

	// Literal types in object types
	type Obj = {
		a: 1;
		b: 'x';
	};
</script>
