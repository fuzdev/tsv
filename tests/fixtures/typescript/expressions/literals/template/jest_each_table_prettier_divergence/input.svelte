<script lang="ts">
	test.each`
		a | b | expected
		${1} | ${2} | ${3}
		${100} | ${200} | ${300}
	`('adds $a + $b', ({ a, b, expected }) => {
		expect(a + b).toBe(expected);
	});
</script>
