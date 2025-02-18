
bl_info = {
    "name": "Bake and export",
    "blender": (4, 1, 0),
    "category": "Object",
}

import bpy
import os

class BakeAndExport(bpy.types.Operator):
    bl_idname = "object.bake_and_export"        # Unique identifier for buttons and menu items to reference.
    bl_label = "Bake and Export"         # Display name in the interface.
    bl_options = {'REGISTER'}  # Enable undo for the operator.

    def execute(self, context):        # execute() is called when running the operator.
        collection_name = "Collection"  # Change this to your collection name
        output_directory = "C:/dev/rust/rust development/wgpu/New folder/cityscaper/res"  # Change this to your desired output directory

        collection = bpy.data.collections.get(collection_name)
        if collection is None:
            print("Collection not found.")
            return

        bpy.ops.object.select_all(action='DESELECT')

        for obj in collection.objects:
            obj.select_set(True)
            bake_lighting(obj)
            obj_name = obj.name.lower()
            filepath = os.path.join(output_directory, obj_name + ".obj")
            export_obj(obj, filepath)
            obj.select_set(False)
            
        return {'FINISHED'}

def menu_func(self, context):
    self.layout.operator(BakeAndExport.bl_idname)

    
def register():
    bpy.utils.register_class(BakeAndExport)
    bpy.types.VIEW3D_MT_object.append(menu_func)  # Adds the new operator to an existing menu.
    
def unregister():
    bpy.utils.unregister_class(BakeAndExport)

# Function to bake lighting for an object
def bake_lighting(obj):
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)

    bpy.ops.object.bake(
        type='DIFFUSE',
        target='VERTEX_COLORS',
    )

# Function to export object as .obj file
def export_obj(obj, filepath):
    bpy.ops.wm.obj_export(
        filepath=filepath, 
        check_existing=False, 
        export_selected_objects=True, 
        export_colors=True, 
        export_materials=False, 
        export_triangulated_mesh=True, 
        filter_glob='*.obj;'
    )


if __name__ == "__main__":
    register()
