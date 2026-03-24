package mindustry.gen;

import arc.func.Prov;
import arc.struct.IntMap;
import arc.struct.ObjectMap;

public class EntityMapping {
  public static IntMap<String> customIdMap = new IntMap<>();

  public static Prov[] idMap = new Prov[256];

  public static ObjectMap<String, Prov> nameMap = new ObjectMap<>();

  static {
    idMap[0] = mindustry.gen.UnitEntityLegacyAlpha::new;
    nameMap.put("alpha", mindustry.gen.UnitEntityLegacyAlpha::new);
    nameMap.put("UnitEntityLegacyAlpha", mindustry.gen.UnitEntityLegacyAlpha::new);
    nameMap.put("unit-entity-legacy-alpha", mindustry.gen.UnitEntityLegacyAlpha::new);
    idMap[29] = mindustry.gen.LegsUnitLegacyArkyid::new;
    nameMap.put("LegsUnitLegacyArkyid", mindustry.gen.LegsUnitLegacyArkyid::new);
    nameMap.put("legs-unit-legacy-arkyid", mindustry.gen.LegsUnitLegacyArkyid::new);
    nameMap.put("arkyid", mindustry.gen.LegsUnitLegacyArkyid::new);
    idMap[30] = mindustry.gen.UnitEntityLegacyBeta::new;
    nameMap.put("UnitEntityLegacyBeta", mindustry.gen.UnitEntityLegacyBeta::new);
    nameMap.put("unit-entity-legacy-beta", mindustry.gen.UnitEntityLegacyBeta::new);
    nameMap.put("beta", mindustry.gen.UnitEntityLegacyBeta::new);
    idMap[2] = mindustry.gen.BlockUnitUnit::new;
    nameMap.put("block", mindustry.gen.BlockUnitUnit::new);
    nameMap.put("BlockUnitUnit", mindustry.gen.BlockUnitUnit::new);
    nameMap.put("block-unit-unit", mindustry.gen.BlockUnitUnit::new);
    idMap[24] = mindustry.gen.LegsUnit::new;
    nameMap.put("collaris", mindustry.gen.LegsUnit::new);
    nameMap.put("corvus", mindustry.gen.LegsUnit::new);
    nameMap.put("atrax", mindustry.gen.LegsUnit::new);
    nameMap.put("anthicus", mindustry.gen.LegsUnit::new);
    nameMap.put("LegsUnit", mindustry.gen.LegsUnit::new);
    nameMap.put("legs-unit", mindustry.gen.LegsUnit::new);
    nameMap.put("merui", mindustry.gen.LegsUnit::new);
    nameMap.put("cleroi", mindustry.gen.LegsUnit::new);
    nameMap.put("tecta", mindustry.gen.LegsUnit::new);
    idMap[45] = mindustry.gen.ElevationMoveUnit::new;
    nameMap.put("ElevationMoveUnit", mindustry.gen.ElevationMoveUnit::new);
    nameMap.put("elevation-move-unit", mindustry.gen.ElevationMoveUnit::new);
    nameMap.put("elude", mindustry.gen.ElevationMoveUnit::new);
    idMap[3] = mindustry.gen.UnitEntity::new;
    nameMap.put("eclipse", mindustry.gen.UnitEntity::new);
    nameMap.put("obviate", mindustry.gen.UnitEntity::new);
    nameMap.put("flare", mindustry.gen.UnitEntity::new);
    nameMap.put("avert", mindustry.gen.UnitEntity::new);
    nameMap.put("zenith", mindustry.gen.UnitEntity::new);
    nameMap.put("horizon", mindustry.gen.UnitEntity::new);
    nameMap.put("UnitEntity", mindustry.gen.UnitEntity::new);
    nameMap.put("unit-entity", mindustry.gen.UnitEntity::new);
    nameMap.put("antumbra", mindustry.gen.UnitEntity::new);
    idMap[31] = mindustry.gen.UnitEntityLegacyGamma::new;
    nameMap.put("gamma", mindustry.gen.UnitEntityLegacyGamma::new);
    nameMap.put("UnitEntityLegacyGamma", mindustry.gen.UnitEntityLegacyGamma::new);
    nameMap.put("unit-entity-legacy-gamma", mindustry.gen.UnitEntityLegacyGamma::new);
    idMap[46] = mindustry.gen.CrawlUnit::new;
    nameMap.put("renale", mindustry.gen.CrawlUnit::new);
    nameMap.put("CrawlUnit", mindustry.gen.CrawlUnit::new);
    nameMap.put("crawl-unit", mindustry.gen.CrawlUnit::new);
    nameMap.put("latum", mindustry.gen.CrawlUnit::new);
    idMap[4] = mindustry.gen.MechUnit::new;
    nameMap.put("MechUnit", mindustry.gen.MechUnit::new);
    nameMap.put("mech-unit", mindustry.gen.MechUnit::new);
    nameMap.put("scepter", mindustry.gen.MechUnit::new);
    nameMap.put("dagger", mindustry.gen.MechUnit::new);
    nameMap.put("crawler", mindustry.gen.MechUnit::new);
    nameMap.put("fortress", mindustry.gen.MechUnit::new);
    nameMap.put("vela", mindustry.gen.MechUnit::new);
    nameMap.put("mace", mindustry.gen.MechUnit::new);
    nameMap.put("reign", mindustry.gen.MechUnit::new);
    idMap[36] = mindustry.gen.BuildingTetherPayloadUnit::new;
    nameMap.put("BuildingTetherPayloadUnit", mindustry.gen.BuildingTetherPayloadUnit::new);
    nameMap.put("building-tether-payload-unit", mindustry.gen.BuildingTetherPayloadUnit::new);
    nameMap.put("manifold", mindustry.gen.BuildingTetherPayloadUnit::new);
    nameMap.put("assemblyDrone", mindustry.gen.BuildingTetherPayloadUnit::new);
    nameMap.put("assembly-drone", mindustry.gen.BuildingTetherPayloadUnit::new);
    idMap[5] = mindustry.gen.PayloadUnit::new;
    nameMap.put("quell", mindustry.gen.PayloadUnit::new);
    nameMap.put("PayloadUnit", mindustry.gen.PayloadUnit::new);
    nameMap.put("payload-unit", mindustry.gen.PayloadUnit::new);
    nameMap.put("disrupt", mindustry.gen.PayloadUnit::new);
    nameMap.put("emanate", mindustry.gen.PayloadUnit::new);
    nameMap.put("mega", mindustry.gen.PayloadUnit::new);
    nameMap.put("evoke", mindustry.gen.PayloadUnit::new);
    nameMap.put("incite", mindustry.gen.PayloadUnit::new);
    idMap[6] = mindustry.gen.Building::new;
    nameMap.put("Building", mindustry.gen.Building::new);
    nameMap.put("building", mindustry.gen.Building::new);
    idMap[7] = mindustry.gen.Bullet::new;
    nameMap.put("Bullet", mindustry.gen.Bullet::new);
    nameMap.put("bullet", mindustry.gen.Bullet::new);
    idMap[8] = mindustry.gen.Decal::new;
    nameMap.put("Decal", mindustry.gen.Decal::new);
    nameMap.put("decal", mindustry.gen.Decal::new);
    idMap[9] = mindustry.gen.EffectState::new;
    nameMap.put("EffectState", mindustry.gen.EffectState::new);
    nameMap.put("effect-state", mindustry.gen.EffectState::new);
    idMap[10] = mindustry.gen.Fire::new;
    nameMap.put("Fire", mindustry.gen.Fire::new);
    nameMap.put("fire", mindustry.gen.Fire::new);
    idMap[11] = mindustry.gen.LaunchCore::new;
    nameMap.put("LaunchCore", mindustry.gen.LaunchCore::new);
    nameMap.put("launch-core", mindustry.gen.LaunchCore::new);
    idMap[12] = mindustry.gen.Player::new;
    nameMap.put("Player", mindustry.gen.Player::new);
    nameMap.put("player", mindustry.gen.Player::new);
    idMap[28] = mindustry.gen.PosTeam::new;
    nameMap.put("PosTeam", mindustry.gen.PosTeam::new);
    nameMap.put("pos-team", mindustry.gen.PosTeam::new);
    idMap[42] = mindustry.gen.PowerGraphUpdater::new;
    nameMap.put("PowerGraphUpdater", mindustry.gen.PowerGraphUpdater::new);
    nameMap.put("power-graph-updater", mindustry.gen.PowerGraphUpdater::new);
    idMap[13] = mindustry.gen.Puddle::new;
    nameMap.put("Puddle", mindustry.gen.Puddle::new);
    nameMap.put("puddle", mindustry.gen.Puddle::new);
    idMap[35] = mindustry.gen.WorldLabel::new;
    nameMap.put("WorldLabel", mindustry.gen.WorldLabel::new);
    nameMap.put("world-label", mindustry.gen.WorldLabel::new);
    idMap[14] = mindustry.gen.WeatherState::new;
    nameMap.put("WeatherState", mindustry.gen.WeatherState::new);
    nameMap.put("weather-state", mindustry.gen.WeatherState::new);
    idMap[15] = mindustry.gen.LaunchPayload::new;
    nameMap.put("LaunchPayload", mindustry.gen.LaunchPayload::new);
    nameMap.put("launch-payload", mindustry.gen.LaunchPayload::new);
    idMap[39] = mindustry.gen.TimedKillUnit::new;
    nameMap.put("missile", mindustry.gen.TimedKillUnit::new);
    nameMap.put("TimedKillUnit", mindustry.gen.TimedKillUnit::new);
    nameMap.put("timed-kill-unit", mindustry.gen.TimedKillUnit::new);
    idMap[16] = mindustry.gen.UnitEntityLegacyMono::new;
    nameMap.put("UnitEntityLegacyMono", mindustry.gen.UnitEntityLegacyMono::new);
    nameMap.put("unit-entity-legacy-mono", mindustry.gen.UnitEntityLegacyMono::new);
    nameMap.put("mono", mindustry.gen.UnitEntityLegacyMono::new);
    idMap[17] = mindustry.gen.MechUnitLegacyNova::new;
    nameMap.put("nova", mindustry.gen.MechUnitLegacyNova::new);
    nameMap.put("MechUnitLegacyNova", mindustry.gen.MechUnitLegacyNova::new);
    nameMap.put("mech-unit-legacy-nova", mindustry.gen.MechUnitLegacyNova::new);
    idMap[26] = mindustry.gen.PayloadUnitLegacyOct::new;
    nameMap.put("oct", mindustry.gen.PayloadUnitLegacyOct::new);
    nameMap.put("PayloadUnitLegacyOct", mindustry.gen.PayloadUnitLegacyOct::new);
    nameMap.put("payload-unit-legacy-oct", mindustry.gen.PayloadUnitLegacyOct::new);
    idMap[18] = mindustry.gen.UnitEntityLegacyPoly::new;
    nameMap.put("poly", mindustry.gen.UnitEntityLegacyPoly::new);
    nameMap.put("UnitEntityLegacyPoly", mindustry.gen.UnitEntityLegacyPoly::new);
    nameMap.put("unit-entity-legacy-poly", mindustry.gen.UnitEntityLegacyPoly::new);
    idMap[19] = mindustry.gen.MechUnitLegacyPulsar::new;
    nameMap.put("MechUnitLegacyPulsar", mindustry.gen.MechUnitLegacyPulsar::new);
    nameMap.put("mech-unit-legacy-pulsar", mindustry.gen.MechUnitLegacyPulsar::new);
    nameMap.put("pulsar", mindustry.gen.MechUnitLegacyPulsar::new);
    idMap[23] = mindustry.gen.PayloadUnitLegacyQuad::new;
    nameMap.put("quad", mindustry.gen.PayloadUnitLegacyQuad::new);
    nameMap.put("PayloadUnitLegacyQuad", mindustry.gen.PayloadUnitLegacyQuad::new);
    nameMap.put("payload-unit-legacy-quad", mindustry.gen.PayloadUnitLegacyQuad::new);
    idMap[32] = mindustry.gen.MechUnitLegacyQuasar::new;
    nameMap.put("quasar", mindustry.gen.MechUnitLegacyQuasar::new);
    nameMap.put("MechUnitLegacyQuasar", mindustry.gen.MechUnitLegacyQuasar::new);
    nameMap.put("mech-unit-legacy-quasar", mindustry.gen.MechUnitLegacyQuasar::new);
    idMap[20] = mindustry.gen.UnitWaterMove::new;
    nameMap.put("retusa", mindustry.gen.UnitWaterMove::new);
    nameMap.put("UnitWaterMove", mindustry.gen.UnitWaterMove::new);
    nameMap.put("unit-water-move", mindustry.gen.UnitWaterMove::new);
    nameMap.put("cyerce", mindustry.gen.UnitWaterMove::new);
    nameMap.put("omura", mindustry.gen.UnitWaterMove::new);
    nameMap.put("navanax", mindustry.gen.UnitWaterMove::new);
    nameMap.put("oxynoe", mindustry.gen.UnitWaterMove::new);
    nameMap.put("sei", mindustry.gen.UnitWaterMove::new);
    nameMap.put("aegires", mindustry.gen.UnitWaterMove::new);
    nameMap.put("bryde", mindustry.gen.UnitWaterMove::new);
    nameMap.put("minke", mindustry.gen.UnitWaterMove::new);
    nameMap.put("risso", mindustry.gen.UnitWaterMove::new);
    idMap[21] = mindustry.gen.LegsUnitLegacySpiroct::new;
    nameMap.put("spiroct", mindustry.gen.LegsUnitLegacySpiroct::new);
    nameMap.put("LegsUnitLegacySpiroct", mindustry.gen.LegsUnitLegacySpiroct::new);
    nameMap.put("legs-unit-legacy-spiroct", mindustry.gen.LegsUnitLegacySpiroct::new);
    idMap[43] = mindustry.gen.TankUnit::new;
    nameMap.put("precept", mindustry.gen.TankUnit::new);
    nameMap.put("conquer", mindustry.gen.TankUnit::new);
    nameMap.put("vanquish", mindustry.gen.TankUnit::new);
    nameMap.put("locus", mindustry.gen.TankUnit::new);
    nameMap.put("stell", mindustry.gen.TankUnit::new);
    nameMap.put("TankUnit", mindustry.gen.TankUnit::new);
    nameMap.put("tank-unit", mindustry.gen.TankUnit::new);
    idMap[33] = mindustry.gen.LegsUnitLegacyToxopid::new;
    nameMap.put("toxopid", mindustry.gen.LegsUnitLegacyToxopid::new);
    nameMap.put("LegsUnitLegacyToxopid", mindustry.gen.LegsUnitLegacyToxopid::new);
    nameMap.put("legs-unit-legacy-toxopid", mindustry.gen.LegsUnitLegacyToxopid::new);
  }

  /**
   * Use this method for obtaining a classId for custom modded unit types. Only call this once for each type. Modded types should return this id in their overridden classId method.
   */
  public static int register(String name, Prov constructor) {
    int next = arc.util.Structs.indexOf(idMap, v -> v == null);
    idMap[next] = constructor;
    nameMap.put(name, constructor);
    customIdMap.put(next, name);
    return next;
  }

  public static Prov map(int id) {
    return idMap[id];
  }

  public static Prov map(String name) {
    return nameMap.get(name);
  }
}
