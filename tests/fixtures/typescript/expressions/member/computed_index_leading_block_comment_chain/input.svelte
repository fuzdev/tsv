<script lang="ts">
	// a block leading a computed index prints inline against it, so the chain lays out
	// by width whatever paren the index was written in
	a.b()[/* c1 */ c as D].e();
	a.b()[/* c2 */ () => {}].e();
	x[/* c3 */ y.p].m().n();
	q.r()[/* c4 */ y.p].m();
	x[/* c5 */ y.p].m();
	q.r()[/* c6 */ (a as B).p].m();
</script>
