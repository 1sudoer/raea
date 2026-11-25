// raea/people: the people of a place as data and as days (docs/character.feature CH50-CH53). A project lists its
// roles (a profession, a rank, a household part: who fills it, what they wear and carry, how they travel, their day
// and habits) and its places; the SDK draws the people, each from a seed (name, sex, age, traits, face and body
// numbers, hair, dress, gait), gives each a day (blocks at real places, trips between them by mode, habits), and
// says where anyone is at any minute, at three levels of detail by distance from the camera.
//
//   import { populate, cityDay, snapshot, react } from "raea/people";
//   const people = populate({ roles, places, folk, mix: { baker: 3, housewife: 40 }, seed: "leipziger" });
//   const day = cityDay(people, roles, places, { key: "1917-12-03", weekday: 1 });
//   const now = snapshot(day, 17 * 60 + 30, cameraAt, { near: 150, middle: 1500 });
export * from "./people/types";
export { rng, hash, type Rng } from "./people/rng";
export { makePerson, gaitOf, leaned, TRAITS, type Folk, type Names } from "./people/persona";
export { dayOf, whereAt, walkSpeed, straight, SPEED, WALK_UNDER, type DayInput } from "./people/day";
export { populate, cityDay, snapshot, census, streetMix, levelAt, react, type CityInput, type Mix, type Day, type Event } from "./people/city";
export { Grid, gridOf } from "./people/near";
export { passersBy, busyAt, STREET_HOURS, type Walk, type Passer, type FlowInput } from "./people/flow";
