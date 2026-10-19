// A record used to describe the implementation's point parameter.
type Point = @{
    let x: float;
    let y: float;
};

// Squared distance from the origin.
let length_squared: $[](point: Point) -> float;
let point: Point;
let result: float;
