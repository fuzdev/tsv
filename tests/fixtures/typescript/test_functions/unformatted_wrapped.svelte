<script lang="ts">
	// Core test functions - stay on one line even if long
	it(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	test(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	describe(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);

	// With skip/only variants
	it.skip(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	test.skip(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	test.only(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);

	// Playwright 3.7 additions
	test.fixme(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	test.step(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);

	// Focus/skip prefixes
	xit(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);
	fit(
		'does something really long and complicated so I have to write a very long name for the test',
		() => {},
	);

	// With timeout (third argument)
	it(
		'does something really long and complicated so I have to write a very long name',
		() => {},
		5000,
	);
</script>
