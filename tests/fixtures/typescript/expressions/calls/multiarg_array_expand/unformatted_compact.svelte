<script lang="ts">
	// At 101 effective chars, Prettier expands ALL args, keeps last array inline
	fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',['x'],['value0000001','value0000002','value0000003']);

	// At 100 effective chars, stays on one line
	fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',['x'],['value0000001','value0000002','value0000003']);

	// Three args with arrays - expands all when over width
	test('value000000000000001',['value000000000000002'],['value000000000000003','value000000000000004','value000000000000005']);

	// Four args where last arg is not array - different pattern (not same-type)
	process('config',['preset'],['a','b','c','d','e','f','g','h','i','j','k','l','m','n'],callback);

	// Trailing comment on array arg pushes line to 101 chars - stays inline
	process('config',['preset'],['a','b','c','d','e','f','g','h','i','j','k','l','m','n'], // trailing comment xxxxx
);

	// Type assertion with trailing comment - array stays inline
	process('config',['preset'],['a','b','c','d','e','f','g','h','i','j','k','l'] as const, // trailing comment xxxxxx
);

	// Object with trailing comment - stays inline
	configure('settings',['defaults'],{aa:1,bb:2,cc:3,dd:4,ee:5,ff:6,gg:7,hh:8,ii:9}, // trailing comment xxxxxxxxxxxx
);

	// Function last arg with trailing comment - expands all args
	process('config',['preset'],()=>{return 1;}, // trailing comment
);
</script>
