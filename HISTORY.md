0.2.3 / 2016/05/02
==================

  * remove itertools dependency by reimplementing fold1
  * drop row lock before checking whether newly scouted solution is the best so far
  * minor refactoring to implement clippy recommendations

0.2.2 / 2016/03/31
==================

  * fix bug that caused hive initialization to run one candidate at a time
  * fix bug that caused all new candidates to block while scouting
  * send best solution immediately when setting a sender on a Hive

0.2.1 / 2016/03/27
==================

  * remove a priori requirement for Context to be 'static
  * add context(), set_sender(Sender), and run_forever() methods to Hive

0.2.0 / 2016/03/27
==================

  * switch to context-oriented interface for complicated problems

0.1.2 / 2016/03/25
==================

  * prevent workers from working on solutions that are being scouted
  * add a bunch of comments

0.1.1 / 2016-03-24
==================

  * make license in Cargo.toml clearer
  * fix bug where solution builder lock would be held too long
  * prevent observers from working on solutions that are being scouted

0.1.0 / 2016-03-23
==================

  * initial release
