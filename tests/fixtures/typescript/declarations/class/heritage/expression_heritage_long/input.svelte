<script lang="ts">
	// Short — header stays inline
	const Short = class extends Base implements A, B {
		x = 1;
	};

	// At 100 chars — header stays inline, empty body hugs
	const Fits = class extends Base implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccccc {};

	// At 101 chars — header breaks; empty body brace stays on the implements line
	const Overr = class
		extends Base
		implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccccc {};

	// Over width with a non-empty body — header breaks; opening brace on its own line
	const Bodyyyy = class
		extends Base
		implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccccc
	{
		x = 1;
	};

	// Many long implements — both groups break, each implements item per line
	const ManyLong = class
		extends Base
		implements
			Aaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbb,
			Cccccccccccccccc,
			Dddddddddddddddd,
			Eeeeeeeeeeeeeeee {};
</script>
