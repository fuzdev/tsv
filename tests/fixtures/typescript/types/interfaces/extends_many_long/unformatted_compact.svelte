<script lang="ts">
	// Short — stays inline
	interface Short extends A, B, C { a: string; }

	// At 100 chars — stays inline
	interface Fits extends Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Ddddddddddddddddddddd {}

	// At 101 chars — outer group breaks, types stay on extends line
	interface Over extends Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Dddddddddddddddddddddd {}

	// Many types — both groups break, each type per line
	interface ManyLong extends Aaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbb, Cccccccccccccccc, Dddddddddddddddd, Eeeeeeeeeeeeeeee {}

	// Single type param with extends — type param stays inline
	interface WithTypeParam<T> extends Aaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbb, Ccccccccccccccc, Ddddddddddddddd {}
</script>
