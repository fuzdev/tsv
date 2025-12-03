<script>
/* short re-export - stays inline */
export {
  aaaa, bbbb, cccc, dddd, eeee, ffff
} from 'short';

/* long re-export - wraps */
export {gggggggggg, hhhhhhhhhh, iiiiiiiiii, jjjjjjjjjj, kkkkkkkkkk, llllllllll, mmmmmmmmmm} from 'module';

/* long re-export with renaming - wraps */
export {nnn as n1, ooo as o1, ppp as p1, qqq as q1, rrr as r1, sss as s1, ttt as t1, uuu as u1} from 'rn';

/* short named export (no from) - stays inline */
export {vvvvvvvvvv, wwwwwwwwww, xxxxxxxxxx, yyyyyyyyyy, zzzzzzzzzz};

/* long named export (no from) - also wraps */
export {aaaaaaaaab, bbbbbbbbbc, cccccccccd, ddddddddde, eeeeeeeeef, ffffffffffg, gggggggggh, hhhhhhhhi};
</script>
