<script lang="ts">
	// 100 boundary flat - stays inline (100 chars with indent)
	interface A extends Omit<Partial<BBBBBBBBBBBBBBBBBBBBBBB>, 'aaaaaaaa' | 'bbbbbbbb' | 'cccccccc'> {
		a?: Partial<BBBBBBBBBBBBBBBBBBBBBBB['aaaaaaaa']>;
	}

	// 101 boundary break - type args break at commas
	interface B extends Omit<
		Partial<BBBBBBBBBBBBBBBBBBBBBBBB>,
		'aaaaaaaa' | 'bbbbbbbb' | 'cccccccc'
	> {
		a?: Partial<BBBBBBBBBBBBBBBBBBBBBBBB['aaaaaaaa']>;
	}

	// Class extends - same behavior (101 boundary)
	class C extends Omit<
		Partial<BBBBBBBBBBBBBBBBBBBBBBBBBBBB>,
		'aaaaaaaa' | 'bbbbbbbb' | 'cccccccc'
	> {
		a?: Partial<BBBBBBBBBBBBBBBBBBBBBBBBBBBB['aaaaaaaa']>;
	}

	// Class implements - same behavior (101 boundary)
	class D implements Omit<
		Partial<BBBBBBBBBBBBBBBBBBBBBBBBB>,
		'aaaaaaaa' | 'bbbbbbbb' | 'cccccccc'
	> {
		a?: string;
	}

	// Short control
	interface E extends Omit<Partial<B>, 'a' | 'b' | 'c'> {
		a?: Partial<B['a']>;
	}
</script>
