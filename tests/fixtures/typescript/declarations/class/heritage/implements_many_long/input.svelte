<script lang="ts">
	// Short — stays inline
	class Short implements A, B, C {
		a = 0;
	}

	// At 100 chars — stays inline
	class Fits implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Dddddddddddddddddddddd {}

	// At 101 chars — outer group breaks, types stay on implements line
	class Over
		implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Ddddddddddddddddddddddd {}

	// Many types — both groups break, each type per line
	class ManyLong
		implements
			Aaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbb,
			Cccccccccccccccc,
			Dddddddddddddddd,
			Eeeeeeeeeeeeeeee {}

	// Single type param with implements — type param stays inline
	class WithTypeParam<T>
		implements Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Ddddddddddddddd {}

	// Extends + many implements — both heritage clauses break
	class Combined
		extends Base
		implements
			Aaaaaaaaaaaaaaaa,
			Bbbbbbbbbbbbbbbb,
			Cccccccccccccccc,
			Dddddddddddddddd,
			Eeeeeeeeeeeeeeee {}
</script>
