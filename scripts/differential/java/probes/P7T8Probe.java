package app.freerouting.autoroute.pipeline;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.settings.RouterSettings;
import java.util.Map;
import java.util.SortedSet;

/**
 * Plan 7 Task 8 probe: the one door into {@code AutorouteConnectionRouter.route} in full — steps
 * 1-8, not the 1-5 slice {@code P6T1.route} inlines.
 *
 * <h2>Why a probe and not a call from {@code P6T1}</h2>
 *
 * <p>{@code AutorouteConnectionRouter} is a package-private class in {@code
 * app.freerouting.autoroute.pipeline} and {@code BatchAutorouter.autorouteItem} (:507-514), the
 * only public-ish way in, is package-private too. {@code P6T1} declares {@code package
 * app.freerouting.autoroute.maze} (it needs the engine's package-private members), so it cannot
 * reach either. This class declares the pipeline package and forwards. It adds <b>no logic</b>:
 * every value below is read straight off {@code BatchAutorouter}'s own {@code RoutingJob}
 * constructor (:110-122), so the router this builds is the router the headless pipeline builds.
 *
 * <h2>The one thing this probe cannot do: disable the 1000 ms budget</h2>
 *
 * <p>Controller ruling AI asks every {@code p7t*} parity run to disable {@code
 * TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} on <b>both</b> sides, and the task brief suggested
 * reflecting the field to 0. <b>That is not possible</b>, and the driver says so rather than
 * pretending: {@code AutorouteConnectionRouter.java:22} declares {@code private static final int
 * TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000}, a compile-time constant, so {@code javac} inlines it
 * at both call sites. {@code javap -c} on the shipping jar shows {@code sipush 1000} immediately
 * before each {@code invokevirtual RoutingBoard.optChangedArea}; no reflective write to the field
 * can change the bytecode.
 *
 * <p>The port answers this the other way round, which is stronger evidence than a matched
 * constant would have been: the Rust twin runs {@code --steps=1-8} with {@code
 * RouterBudget::disabled()}, i.e. <b>no</b> limit, against this side's live 1000 ms one. A MATCH
 * then <em>proves</em> the Java limit never trips on the corpus — had it tripped anywhere, the
 * jar's sweep would have stopped early and the two boards would have diverged. See
 * {@code p6t1.rs}'s module comment.
 */
public final class P7T8Probe {

  private P7T8Probe() {}

  /**
   * {@code new BatchAutorouter(RoutingJob)} (BatchAutorouter.java:110-122) with the four values
   * it derives written out, because {@code RoutingJob} is Plan 8's and this driver has none.
   *
   * <p>{@code thread} is {@code null}: {@code P6T1} already passes {@code null} for {@code
   * initAutoroute}'s {@code Stoppable}, {@code AutorouteConnectionRouter} only ever forwards
   * {@code router.thread} to {@code initAutoroute} (:80) and {@code optChangedArea} (:108, :228),
   * and both accept it — the Rust twin's {@code StopCheck} is the never-stopping one for the same
   * reason.
   */
  public static BatchAutorouter newRouter(RoutingBoard board, RouterSettings settings) {
    return new BatchAutorouter(
        null,
        board,
        settings,
        // :115.
        !settings.isFanoutEnabled(),
        // :116.
        true,
        // :117.
        settings.getStartRipupCosts(),
        // :118-120.
        settings.tracePullTightAccuracy != null ? settings.tracePullTightAccuracy : 500);
  }

  /** {@code BatchAutorouter.autorouteItem} (:507-514) — one delegation to {@code route}. */
  public static AutorouteAttemptResult routeFull(
      BatchAutorouter router,
      Item item,
      int routeNetNo,
      SortedSet<Item> rippedItemList,
      Map<Item, Integer> ripupCosts,
      int ripupPassNo) {
    return router.autorouteItem(item, routeNetNo, rippedItemList, ripupCosts, ripupPassNo);
  }

  /** {@code BatchAutorouter.getTracePullTightAccuracy} (:180-182), for the driver's header line. */
  public static int tracePullTightAccuracy(BatchAutorouter router) {
    return router.getTracePullTightAccuracy();
  }
}
