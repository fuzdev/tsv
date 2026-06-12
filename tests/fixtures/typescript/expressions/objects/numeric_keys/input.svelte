<script lang="ts">
	// non-decimal and bigint object keys decode like standalone literals
	const a = {
		0xb_b: 'value1',
		0b1_1n: 'value2',
		1n: 'value3',
		0x123abcdef456abcdef: 'value4',
		1_8_7: 'value5',
	};

	// literal class member keys (string, numeric, bigint)
	class A {
		'a-b'() {}
		0xb_b = 1;
		static 0 = 2;
		1n = 3;
	}

	// literal keys after accessor and method modifiers
	class B {
		get 0x1() {
			return 1;
		}
		set 's-s'(v) {}
		async 2() {}
		*3() {}
	}
</script>
