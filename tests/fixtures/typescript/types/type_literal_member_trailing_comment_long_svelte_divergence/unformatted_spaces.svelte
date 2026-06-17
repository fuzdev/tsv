<script lang="ts">
	type Props =  {
		// union at exactly 100 columns, no comment — stays inline
		a?:   AAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBB   |   CCCCCCCCCCCCCCCCCCCCCCCCC   |   undefined  ;
		// union at 101 columns, no comment — breaks into leading-pipe multiline
		b?:   AAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBB   |   CCCCCCCCCCCCCCCCCCCCCCCCCC   |   undefined  ;
		// 100-column union plus a trailing comment past 100 — stays inline (comment excluded from fits)
		c?:   AAAAAAAAAAAAAAAAAAAAAAAA   |   BBBBBBBBBBBBBBBBBBBBBBBB   |   CCCCCCCCCCCCCCCCCCCCCCCCC   |   undefined  ;  // trailing comment that pushes the line well past one hundred columns
	} ;
</script>
