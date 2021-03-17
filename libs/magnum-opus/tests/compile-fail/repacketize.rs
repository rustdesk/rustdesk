extern crate opus;

fn main() {
    let mut rp = opus::Repacketizer::new().unwrap();
    let mut wip = rp.begin().cat_move(
        &[1, 2, 3]
        //~^ ERROR borrowed value does not live long enough
    ).unwrap();
    wip.out(&mut []);
}
