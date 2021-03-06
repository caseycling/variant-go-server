use crate::game::{Board, GroupVec, NPlusOne, Point, Visibility, VisibilityBoard};

pub enum NPlusOneResult {
    ExtraTurn,
    Nothing,
}

pub fn check(
    points_played: &GroupVec<Point>,
    board: &Board,
    mut visibility: Option<&mut VisibilityBoard>,
    rule: &NPlusOne,
) -> NPlusOneResult {
    let mut line_points = Vec::new();

    let mut matched = false;

    for &point_played in points_played {
        let color = board.get_point(point_played);

        let add_point = |line_points: &mut Vec<Point>, p: Point| {
            if board.get_point(p) == color {
                line_points.push(p);
                false
            } else {
                true
            }
        };

        // Vertical ///////////////////////////////////////////////////////////

        for y in (0..point_played.1).rev() {
            if add_point(&mut line_points, (point_played.0, y)) {
                break;
            }
        }

        for y in point_played.1..board.height {
            if add_point(&mut line_points, (point_played.0, y)) {
                break;
            }
        }

        let vertical_match = line_points.len() == rule.length as usize;

        if vertical_match {
            if let Some(visibility) = visibility.as_mut() {
                for &p in &line_points {
                    *visibility.point_mut(p) = Visibility::new();
                }
            }
        }

        line_points.clear();

        // Horizontal /////////////////////////////////////////////////////////

        for x in (0..point_played.0).rev() {
            if add_point(&mut line_points, (x, point_played.1)) {
                break;
            }
        }

        for x in point_played.0..board.width {
            if add_point(&mut line_points, (x, point_played.1)) {
                break;
            }
        }

        let horizontal_match = line_points.len() == rule.length as usize;

        if horizontal_match {
            if let Some(visibility) = visibility.as_mut() {
                for &p in &line_points {
                    *visibility.point_mut(p) = Visibility::new();
                }
            }
        }

        line_points.clear();

        // Diagonal top left - bottom right ///////////////////////////////////

        let mut p = point_played;
        while p.0 > 0 && p.1 > 0 {
            p.0 -= 1;
            p.1 -= 1;

            if add_point(&mut line_points, p) {
                break;
            }
        }

        let mut p = point_played;
        while board.point_within(p) {
            if add_point(&mut line_points, p) {
                break;
            }

            p.0 += 1;
            p.1 += 1;
        }

        let diagonal_tlbr_match = line_points.len() == rule.length as usize;

        if diagonal_tlbr_match {
            if let Some(visibility) = visibility.as_mut() {
                for &p in &line_points {
                    *visibility.point_mut(p) = Visibility::new();
                }
            }
        }

        line_points.clear();

        // Diagonal bottom left - top right ///////////////////////////////////

        let mut p = point_played;
        while p.0 > 0 {
            p.0 -= 1;
            p.1 += 1;

            if !board.point_within(p) {
                break;
            }

            if add_point(&mut line_points, p) {
                break;
            }
        }

        let mut p = point_played;
        while board.point_within(p) {
            if add_point(&mut line_points, p) {
                break;
            }

            if p.1 == 0 {
                break;
            }

            p.0 += 1;
            p.1 -= 1;
        }

        let diagonal_bltr_match = line_points.len() == rule.length as usize;

        if diagonal_bltr_match {
            if let Some(visibility) = visibility.as_mut() {
                for &p in &line_points {
                    *visibility.point_mut(p) = Visibility::new();
                }
            }
        }

        line_points.clear();

        matched = matched
            || vertical_match
            || horizontal_match
            || diagonal_tlbr_match
            || diagonal_bltr_match;
    }

    if matched {
        return NPlusOneResult::ExtraTurn;
    }

    NPlusOneResult::Nothing
}
