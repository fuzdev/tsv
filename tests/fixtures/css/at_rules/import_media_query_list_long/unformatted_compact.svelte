<style>
/* comma list: packs greedily, breaks before the last query at +1 */
@import 'aaaaaaaa.css' screen and (min-width: 400px),print and (max-width: 999px),tv,projection,handheld;

/* one query overflows: breaks internally at +2, trailing comma forces next query to +1 */
@import 'aaaaaaaa.css' screen and (min-width: 400px) and (max-width: 1200px) and (orientation: landscape),tv;

/* internal break +2, then short queries pack greedily at +1 */
@import 'aaaaaaaa.css' screen and (min-width: 400px) and (max-width: 1200px) and (orientation: landscape),tv,print,all;

/* short multi-query: stays inline (both agree, regression guard) */
@import 'aaaaaaaa.css' screen and (min-width: 400px),print;

/* comma-only media-type list (no and/or): also fills, breaks at the comma at +1 */
@import 'list.css' projection,tv,handheld,embossed,speech,aural,braille,tty,screen,print,all;

/* boundary: 101 chars stays inline (the fill finds no needed break) */
@import 'b.css' screen and (min-width: 700px),print and (max-width: 900px),tv,all,aaaaaaaaaaaa;

/* boundary: 102 chars wraps — last query to +1 (one char over the line above) */
@import 'b.css' screen and (min-width: 700px),print and (max-width: 900px),tv,all,aaaaaaaaaaaaa;
</style>
