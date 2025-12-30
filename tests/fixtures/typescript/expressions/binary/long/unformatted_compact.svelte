<script>
	/* short arithmetic - stays inline */
	const short = 1+2+3+4+5;

	/* 100 chars - stays inline */
	const a = 1+2+3+4+5+6+7+8+9+10+11+12+13+14+15+16+17+18+190000;

	/* 101 chars - breaks after = */
	const b = 1+2+3+4+5+6+7+8+9+10+11+12+13+14+15+16+17+18+1900000;

	/* very long - breaks after = AND wraps each operand */
	const verylong = 1+2+3+4+5+6+7+8+9+10+11+12+13+14+15+16+17+18+19+20+21+22+23;

	/* long logical - same behavior */
	const logic = cond_aaaaaaaaaaaaaaaa||cond_bbbbbbbbbbbbbbbbb||cond_cccccccccccccccc||cond_ddddddddddddd||cond_eeeeeeeeee;

	/* long comparison chain */
	const compare = val_aaaaaa>limit_aaaaaa&&val_aaaaaa<limit_bbbbbb&&val_bbbbbbb>=limit_cccccc&&val_bbbbbbb<=limit_dddddd;
</script>
