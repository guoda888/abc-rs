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