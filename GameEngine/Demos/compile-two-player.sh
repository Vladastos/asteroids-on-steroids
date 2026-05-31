#!/bin/bash
# Compiles TwoPlayerDemo.cs together with the engine source files.
# dotnet build excludes Demos/, so we compile manually with mcs.
set -e
cd "$(dirname "$0")"

ENGINE=../Engine

mcs \
  $ENGINE/Core/Entity.cs \
  $ENGINE/Core/ISparseSetEraser.cs \
  $ENGINE/Core/SparseSet.cs \
  $ENGINE/Core/World.cs \
  $ENGINE/Core/ISystem.cs \
  $ENGINE/Core/QueryResults.cs \
  $ENGINE/Core/GameLoop.cs \
  $ENGINE/Components/Transform.cs \
  $ENGINE/Components/Velocity.cs \
  $ENGINE/Components/RigidBody.cs \
  $ENGINE/Components/Collider.cs \
  $ENGINE/Components/Sprite.cs \
  $ENGINE/Components/Health.cs \
  $ENGINE/Components/Tags.cs \
  $ENGINE/Events/EventBus.cs \
  $ENGINE/Events/CollisionEvent.cs \
  $ENGINE/Input/KeyCode.cs \
  $ENGINE/Input/InputSystem.cs \
  $ENGINE/Rendering/Camera.cs \
  $ENGINE/Collision/CollisionShape.cs \
  $ENGINE/Collision/CircleShape.cs \
  $ENGINE/Collision/AABBShape.cs \
  $ENGINE/Collision/PolygonShape.cs \
  $ENGINE/Collision/ContactInfo.cs \
  $ENGINE/Collision/ISpatialIndex.cs \
  $ENGINE/Collision/SpatialGrid.cs \
  $ENGINE/Systems/MovementSystem.cs \
  $ENGINE/Systems/PhysicsSystem.cs \
  $ENGINE/Systems/CollisionSystem.cs \
  $ENGINE/Systems/RenderSystem.cs \
  $ENGINE/Resources/ResourceManager.cs \
  $ENGINE/State/IGameState.cs \
  $ENGINE/State/StateStack.cs \
  TwoPlayerDemo.cs \
  -r:System.Windows.Forms.dll \
  -r:System.Drawing.dll \
  -r:System.Numerics.dll \
  -out:TwoPlayerDemo.exe

echo "Build OK → mono TwoPlayerDemo.exe"
